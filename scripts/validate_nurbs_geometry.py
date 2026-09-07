#!/usr/bin/env python3
"""Compare native NURBS support geometry with pinned STEP B-splines."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any

import sldkit
from capture_step_geometry import _subshapes
from jsonschema import Draft202012Validator
from nurbs_geometry import Nurbs, NurbsValidationError, finite

ROOT = Path(__file__).resolve().parents[1]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def checked_file(root: Path, artifact: dict) -> Path:
    path = Path(artifact["path"])
    if path.is_absolute():
        raise NurbsValidationError("artifact paths must be relative")
    path = (root / path).resolve(strict=True)
    if not path.is_relative_to(root.resolve(strict=True)) or not path.is_file():
        raise NurbsValidationError("artifact path escapes fixture root")
    if (
        path.stat().st_size != artifact["byte_size"]
        or sha256(path) != artifact["sha256"]
    ):
        raise NurbsValidationError(f"artifact size/hash mismatch: {artifact['path']}")
    return path


def unique(items: list[dict], key: str = "id") -> dict:
    result = {}
    for item in items:
        if item[key] in result:
            raise NurbsValidationError(f"duplicate {key}: {item[key]}")
        result[item[key]] = item
    return result


def configuration_carriers(geometry: dict, configuration: str) -> dict:
    configs = [c for c in geometry["configurations"] if c.get("name") == configuration]
    if len(configs) != 1 or not configs[0]["body_ids"]:
        raise NurbsValidationError("configuration membership is missing or ambiguous")
    model = geometry["model"]
    bodies, regions, shells, faces, loops, coedges, edges, carriers = (
        unique(model[key])
        for key in (
            "bodies",
            "regions",
            "shells",
            "faces",
            "loops",
            "coedges",
            "edges",
            "carriers",
        )
    )
    selected = set()
    for body_id in configs[0]["body_ids"]:
        body = bodies[body_id]
        if body.get("transform") is not None:
            raise NurbsValidationError(
                "body transforms are outside this comparison profile"
            )
        for region_id in body["region_ids"]:
            for shell_id in regions[region_id]["shell_ids"]:
                for face_id in shells[shell_id]["face_ids"]:
                    face = faces[face_id]
                    selected.add(face["surface_id"])
                    for loop_id in face["loop_ids"]:
                        for coedge_id in loops[loop_id]["coedge_ids"]:
                            selected.add(
                                edges[coedges[coedge_id]["edge_id"]]["curve_id"]
                            )
    return {
        key: carriers[key]
        for key in sorted(selected)
        if carriers[key]["kind"] == "nurbs"
    }


def step_carriers(path: Path, required_version: str) -> tuple[dict, dict]:
    try:
        import OCP
        from OCP.BRepAdaptor import BRepAdaptor_Curve, BRepAdaptor_Surface
        from OCP.IFSelect import IFSelect_RetDone
        from OCP.STEPControl import STEPControl_Reader
        from OCP.TopAbs import TopAbs_EDGE, TopAbs_FACE
        from OCP.TopoDS import TopoDS
    except ImportError as error:
        raise NurbsValidationError(
            "OCP is required in the validation environment"
        ) from error
    if OCP.__version__ != required_version:
        raise NurbsValidationError(
            "OCP version differs from the oracle's subshape indexing contract"
        )
    reader = STEPControl_Reader()
    if reader.ReadFile(str(path)) != IFSelect_RetDone:
        raise NurbsValidationError("STEP reader rejected reference")
    reader.SetSystemLengthUnit(1.0)  # Imported reference coordinates in millimetres.
    if reader.TransferRoots() <= 0 or reader.OneShape().IsNull():
        raise NurbsValidationError("STEP reference transferred no shape")
    result = {}
    for domain, kind, ctor, cast in (
        ("curve", TopAbs_EDGE, BRepAdaptor_Curve, TopoDS.Edge_s),
        ("surface", TopAbs_FACE, BRepAdaptor_Surface, TopoDS.Face_s),
    ):
        for index, shape in enumerate(_subshapes(reader.OneShape(), kind), 1):
            adaptor = ctor(cast(shape))
            if adaptor.GetType().name != (
                "GeomAbs_BSplineCurve"
                if domain == "curve"
                else "GeomAbs_BSplineSurface"
            ):
                continue
            # BSpline() returns a copy with the shape's location applied.
            result[(domain, index)] = adaptor.BSpline()
    return result, {
        "tool": "OCP STEPControl_Reader/BRepAdaptor/Geom_BSpline D1",
        "version": str(OCP.__version__),
        "length_unit": "millimeter",
    }


def step_definition(domain: str, spline: Any) -> dict:
    def point(p: Any) -> dict:
        return dict(zip(("x", "y", "z"), p.Coord(), strict=True))

    if domain == "curve":
        return {
            "kind": "nurbs",
            "degree": spline.Degree(),
            "periodic": spline.IsPeriodic(),
            "control_points": [
                point(spline.Pole(i)) for i in range(1, spline.NbPoles() + 1)
            ],
            "weights": [spline.Weight(i) for i in range(1, spline.NbPoles() + 1)],
            "knots": [
                spline.Knot(i)
                for i in range(1, spline.NbKnots() + 1)
                for _ in range(spline.Multiplicity(i))
            ],
        }
    result = {"kind": "nurbs", "control_points": [], "weights": []}
    for name, prefix in (("u", "U"), ("v", "V")):
        result.update(
            {
                f"{name}_degree": getattr(spline, f"{prefix}Degree")(),
                f"{name}_periodic": getattr(spline, f"Is{prefix}Periodic")(),
                f"{name}_count": getattr(spline, f"Nb{prefix}Poles")(),
                f"{name}_knots": [
                    getattr(spline, f"{prefix}Knot")(i)
                    for i in range(1, getattr(spline, f"Nb{prefix}Knots")() + 1)
                    for _ in range(getattr(spline, f"{prefix}Multiplicity")(i))
                ],
            }
        )
    for i in range(1, spline.NbUPoles() + 1):
        for j in range(1, spline.NbVPoles() + 1):
            result["control_points"].append(point(spline.Pole(i, j)))
            result["weights"].append(spline.Weight(i, j))
    return result


def step_evaluate(spline: Any, parameters: tuple[float, ...]) -> tuple:
    from OCP.gp import gp_Pnt, gp_Vec

    point = gp_Pnt()
    derivatives = [gp_Vec() for _ in parameters]
    spline.D1(*parameters, point, *derivatives)
    return point.Coord(), tuple(v.Coord() for v in derivatives)


def metric(errors: list[float], tolerance: float) -> dict:
    maximum = max(errors, default=math.inf)
    if not math.isfinite(maximum):
        raise NurbsValidationError("nonfinite comparison measurement")
    return {
        "maximum_error": maximum,
        "allowed_error": tolerance,
        "passed": maximum <= tolerance,
    }


def compare(
    native: Nurbs, reference: Nurbs, evaluate: Any, tolerances: dict, subdivisions: int
) -> dict:
    if native.domain != reference.domain:
        raise NurbsValidationError("carrier domain mismatch")
    layout = [(a.degree, a.count, len(a.knots)) for a in native.axes] == [
        (a.degree, a.count, len(a.knots)) for a in reference.axes
    ]
    if not layout:
        return {"passed": False, "error": "degree/count/knot layout mismatch"}
    checks = {
        "control_points_mm": metric(
            [
                math.dist(a, b)
                for a, b in zip(native.points, reference.points, strict=True)
            ],
            tolerances["position_mm"],
        ),
        "knots": metric(
            [
                abs(a - b)
                for x, y in zip(native.axes, reference.axes, strict=True)
                for a, b in zip(x.knots, y.knots, strict=True)
            ],
            tolerances["parameter"],
        ),
        "normalized_weights": metric(
            [
                abs(a - b)
                for a, b in zip(native.weights, reference.weights, strict=True)
            ],
            tolerances["weight"],
        ),
    }
    # Sampling a differently parameterized representation is not meaningful.
    if not checks["knots"]["passed"]:
        return {"passed": False, "checks": checks, "error": "parameterization mismatch"}
    parameters = native.parameters(subdivisions)
    distances, derivative_errors, derivative_ratios = [], [], []
    samples = []
    for params in parameters:
        actual, actual_d = native.evaluate(params)
        # Affine endpoint mapping avoids an out-of-domain last sample caused by
        # decimal export rounding. The scale also applies to d/d(native parameter).
        mapped = []
        scales = []
        for value, a, b in zip(params, native.axes, reference.axes, strict=True):
            lo, hi = a.knots[a.degree], a.knots[a.count]
            first, last = b.knots[b.degree], b.knots[b.count]
            mapped.append(first + (last - first) * (value - lo) / (hi - lo))
            scales.append((last - first) / (hi - lo))
        expected, expected_d = evaluate(tuple(mapped))
        expected = tuple(map(finite, expected))
        expected_d = tuple(
            tuple(finite(c) * scale for c in v)
            for v, scale in zip(expected_d, scales, strict=True)
        )
        distance = math.dist(actual, expected)
        distances.append(distance)
        errors, allowed = [], []
        for a, b in zip(actual_d, expected_d, strict=True):
            error = math.dist(a, b)
            bound = max(
                tolerances["derivative_absolute_mm"],
                tolerances["derivative_relative"] * math.hypot(*b),
            )
            errors.append(error)
            allowed.append(bound)
            derivative_errors.append(error)
            derivative_ratios.append(error / bound)
        samples.append(
            {
                "native_parameters": params,
                "reference_parameters": mapped,
                "native_point_mm": actual,
                "reference_point_mm": expected,
                "point_distance_mm": distance,
                "native_derivatives": actual_d,
                "reference_derivatives": expected_d,
                "derivative_errors": errors,
                "derivative_allowed_errors": allowed,
            }
        )
    checks["sampled_points_mm"] = metric(distances, tolerances["position_mm"])
    checks["sampled_derivatives"] = metric(derivative_ratios, 1.0) | {
        "maximum_absolute_error": max(derivative_errors)
    }
    return {
        "passed": all(c["passed"] for c in checks.values()),
        "checks": checks,
        "sample_count": len(samples),
        "samples": samples,
    }


def run(oracle_path: Path, root: Path) -> dict:
    oracle = json.loads(oracle_path.read_text())
    schema = json.loads(
        (ROOT / "docs/schemas/nurbs-geometry-oracle.schema.json").read_text()
    )
    Draft202012Validator(schema).validate(oracle)
    # jsonschema's numeric model accepts NaN/Infinity; refuse them explicitly.
    for tolerance in oracle["tolerances"].values():
        if finite(tolerance) <= 0:
            raise NurbsValidationError("tolerances must be finite and positive")
    native_path = checked_file(root, oracle["native"])
    step_path = checked_file(root, oracle["neutral"])
    result = sldkit.decode_geometry_file(native_path).to_dict()
    if result["geometry"] is None or result["status"] not in {"decoded", "partial"}:
        raise NurbsValidationError("native geometry is unavailable")
    geometry = result["geometry"]
    if (
        geometry["source"]["sha256"] != oracle["native"]["sha256"]
        or geometry["source"]["byte_len"] != oracle["native"]["byte_size"]
    ):
        raise NurbsValidationError("decoded native source differs from pinned artifact")
    if geometry["length_unit"] != "millimeter":
        raise NurbsValidationError("native geometry must use millimetres")
    native = configuration_carriers(geometry, oracle["configuration"])
    bindings = unique(oracle["bindings"], "native_id")
    if not native or set(bindings) != set(native):
        raise NurbsValidationError(
            "bindings must cover all selected native NURBS carriers exactly once"
        )
    step, capture = step_carriers(step_path, oracle["reference_tool_version"])
    if sha256(step_path) != oracle["neutral"]["sha256"]:
        raise NurbsValidationError("STEP reference changed during capture")
    refs = [(b["domain"], b["step_subshape_index"]) for b in bindings.values()]
    if len(set(refs)) != len(refs) or set(refs) != set(step):
        raise NurbsValidationError(
            "bindings must cover all STEP NURBS carriers exactly once"
        )
    comparisons = []
    for identity, binding in bindings.items():
        carrier = native[identity]
        domain = binding["domain"]
        if carrier["domain"] != domain or (domain == "surface" and binding["reverse"]):
            raise NurbsValidationError("unsupported carrier binding orientation/domain")
        spline = step[(domain, binding["step_subshape_index"])].Copy()
        if binding["reverse"]:
            spline.Reverse()
        definition = step_definition(domain, spline)
        compared = compare(
            Nurbs.read(domain, carrier["definition"]),
            Nurbs.read(domain, definition),
            lambda p, spline=spline: step_evaluate(spline, p),
            oracle["tolerances"],
            oracle["samples_per_knot_span"],
        )
        comparisons.append(
            {
                "binding": binding,
                "native_provenance": carrier.get("provenance"),
                "native_definition": carrier["definition"],
                "reference_definition": definition,
                **compared,
            }
        )
    return {
        "schema_version": 1,
        "passed": all(c["passed"] for c in comparisons),
        "profile": oracle["profile"],
        "configuration": oracle["configuration"],
        "oracle_sha256": sha256(oracle_path),
        "native": oracle["native"],
        "neutral": oracle["neutral"],
        "capture": capture,
        "decoder": geometry["fidelity"]["decoder"],
        "decoder_version": geometry["fidelity"]["decoder_version"],
        "tolerances": oracle["tolerances"],
        "samples_per_knot_span": oracle["samples_per_knot_span"],
        "comparisons": comparisons,
        "scope": (
            "Support geometry and finite point/first-derivative samples; "
            "not trim correctness or a global geometric error bound."
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("oracle", type=Path)
    parser.add_argument("fixture_root", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        report = run(args.oracle, args.fixture_root)
    except Exception as error:
        # Include dependency/kernel/schema failures in the artifact, never a pass.
        report = {
            "schema_version": 1,
            "passed": False,
            "error": str(error),
            "error_type": type(error).__name__,
        }
    output = json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
    if args.output:
        args.output.write_text(output)
        print(
            json.dumps(
                {
                    "passed": report["passed"],
                    "output": str(args.output),
                    "error": report.get("error"),
                }
            )
        )
    else:
        print(output, end="")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
