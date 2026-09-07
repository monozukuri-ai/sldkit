#!/usr/bin/env python3
"""Diagnose rectangular NURBS boundaries against pinned STEP wire geometry.

Endpoint-derived intervals and imported pcurves do not certify native trim
metadata. The strict gate stays false until that source evidence is validated.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path

import sldkit
import validate_nurbs_geometry as support
from capture_step_geometry import _subshapes
from jsonschema import Draft202012Validator
from nurbs_geometry import Nurbs, NurbsValidationError, finite

ROOT = Path(__file__).resolve().parents[1]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise NurbsValidationError(message)


def vector(value: dict, keys: str) -> tuple:
    return tuple(finite(value[k]) for k in keys)


def scale(values: tuple, factor: float) -> tuple:
    return tuple(finite(x * factor) for x in values)


def line(definition: dict, keys: str):
    require(definition["kind"] == "line", "expected linear carrier")
    origin = vector(definition["origin"], keys)
    direction = vector(definition["direction"], keys)
    require(math.hypot(*direction) > 0, "degenerate line")

    def evaluate(t):
        return (
            tuple(finite(o + d * t) for o, d in zip(origin, direction, strict=True)),
            direction,
        )

    return evaluate


def endpoint_relation(native: list, reference: list, tolerance: float) -> str:
    matches = [
        all(math.dist(a, b) <= tolerance for a, b in zip(native, ordered, strict=True))
        for ordered in (reference, reference[::-1])
    ]
    require(sum(matches) == 1, "endpoint correspondence is missing or ambiguous")
    return "same" if matches[0] else "reversed"


def curve_interval(definition: dict, endpoints: list, tolerance: float):
    """Return an evaluator and DERIVED interval; never write source ranges."""
    require(math.dist(*endpoints) > 2 * tolerance, "degenerate/ambiguous edge")
    if definition["kind"] == "line":
        evaluate = line(definition, "xyz")
        origin, direction = evaluate(0)
        squared = finite(sum(d * d for d in direction))
        require(squared > 0, "degenerate line direction")
        interval = [
            finite(
                sum(
                    (p - o) * d
                    for p, o, d in zip(point, origin, direction, strict=True)
                )
                / squared
            )
            for point in endpoints
        ]
        require(
            all(
                math.dist(evaluate(t)[0], p) <= tolerance
                for t, p in zip(interval, endpoints, strict=True)
            ),
            "vertex is not on its line",
        )
        return evaluate, interval, None
    require(definition["kind"] == "nurbs", "unsupported boundary curve")
    curve = Nurbs.read("curve", definition)
    axis = curve.axes[0]
    interval = [axis.knots[axis.degree], axis.knots[axis.count]]

    def evaluate(t):
        point, (derivative,) = curve.evaluate((t,))
        return point, derivative

    relation = endpoint_relation(
        endpoints, [evaluate(t)[0] for t in interval], tolerance
    )
    return evaluate, interval if relation == "same" else interval[::-1], axis


def bounds(surface: Nurbs) -> list:
    return [(a.knots[a.degree], a.knots[a.count]) for a in surface.axes]


def lift(surface: Nurbs, uv: tuple, derivative: tuple, tolerance: float):
    limits = bounds(surface)
    require(
        all(
            lo - tolerance <= finite(t) <= hi + tolerance
            for t, (lo, hi) in zip(uv, limits, strict=True)
        ),
        "pcurve is outside the surface support",
    )
    # Only absorb endpoint roundoff within the oracle's fixed UV tolerance.
    clamped = tuple(min(hi, max(lo, t)) for t, (lo, hi) in zip(uv, limits, strict=True))
    point, (du, dv) = surface.evaluate(clamped)
    tangent = tuple(
        finite(a * derivative[0] + b * derivative[1])
        for a, b in zip(du, dv, strict=True)
    )
    return point, tangent


def rectangle_check(segments: list, limits: list, tolerance: float) -> dict:
    require(len(segments) == 4, "expected four boundary segments")
    require(all(hi - lo > 2 * tolerance for lo, hi in limits), "ambiguous UV domain")
    sides = []
    for start, end in segments:
        candidates = []
        for fixed in (0, 1):
            moving = 1 - fixed
            for side, bound in enumerate(limits[fixed]):
                if (
                    abs(start[fixed] - bound) <= tolerance
                    and abs(end[fixed] - bound) <= tolerance
                    and all(
                        abs(x - y) <= tolerance
                        for x, y in zip(
                            sorted((start[moving], end[moving])),
                            limits[moving],
                            strict=True,
                        )
                    )
                ):
                    candidates.append((fixed, side))
        require(len(candidates) == 1, "boundary is not a full isoparametric side")
        sides.append(candidates[0])
    require(len(set(sides)) == 4, "duplicate or missing rectangle side")
    closure = max(math.dist(segments[i][1], segments[(i + 1) % 4][0]) for i in range(4))
    require(closure <= tolerance, "UV loop is not closed in traversal order")
    area = finite(sum(a[0] * b[1] - b[0] * a[1] for a, b in segments) / 2)
    return {"maximum_uv_closure_error": closure, "signed_uv_area": area, "sides": sides}


def selected_faces(geometry: dict, configuration: str) -> dict:
    model = geometry["model"]
    tables = {
        name: support.unique(model[name])
        for name in ("bodies", "regions", "shells", "faces", "carriers")
    }
    configs = [c for c in geometry["configurations"] if c["name"] == configuration]
    require(
        len(configs) == 1 and bool(configs[0]["body_ids"]),
        "missing configuration membership",
    )
    result = {}
    for body_id in configs[0]["body_ids"]:
        body = tables["bodies"][body_id]
        require(body.get("transform") is None, "body transforms are unsupported")
        for region_id in body["region_ids"]:
            for shell_id in tables["regions"][region_id]["shell_ids"]:
                for face_id in tables["shells"][shell_id]["face_ids"]:
                    face = tables["faces"][face_id]
                    require(face["shell_id"] == shell_id, "face ownership mismatch")
                    if tables["carriers"][face["surface_id"]]["kind"] == "nurbs":
                        require(face_id not in result, "duplicate face membership")
                        result[face_id] = face
    return result


def native_boundary(
    model: dict, face: dict, position_tolerance: float, uv_tolerance: float
) -> dict:
    tables = {
        name: support.unique(model[name])
        for name in ("loops", "coedges", "edges", "vertices", "points", "carriers")
    }
    surface = Nurbs.read(
        "surface", tables["carriers"][face["surface_id"]]["definition"]
    )
    require(face["sense"] in {"forward", "reversed"}, "unknown face sense")
    require(len(face["loop_ids"]) == 1, "only one rectangular loop is supported")
    loop = tables["loops"][face["loop_ids"][0]]
    ids = loop["coedge_ids"]
    require(loop["face_id"] == face["id"], "loop ownership mismatch")
    require(len(ids) == 4 and len(set(ids)) == 4, "expected four distinct coedges")
    edges = []
    for i, identity in enumerate(ids):
        coedge = tables["coedges"][identity]
        require(coedge["loop_id"] == loop["id"], "coedge ownership mismatch")
        require(
            coedge["next_id"] == ids[(i + 1) % 4]
            and coedge["previous_id"] == ids[(i - 1) % 4],
            "broken next/previous coedge ring",
        )
        require(coedge["sense"] in {"forward", "reversed"}, "unknown coedge sense")
        require(
            coedge.get("use_curve_id") is None, "alternate use curves are unsupported"
        )
        edge = tables["edges"][coedge["edge_id"]]
        vertex_ids = [edge["start_vertex_id"], edge["end_vertex_id"]]
        if coedge["sense"] == "reversed":
            vertex_ids.reverse()
        endpoints = [
            tuple(
                map(
                    finite,
                    tables["points"][tables["vertices"][v]["point_id"]]["position"],
                )
            )
            for v in vertex_ids
        ]
        carrier = tables["carriers"][edge["curve_id"]]
        require(carrier["domain"] == "curve", "boundary carrier domain mismatch")
        evaluate, interval, axis = curve_interval(
            carrier["definition"], endpoints, position_tolerance
        )
        require(len(coedge["pcurves"]) == 1, "expected one pcurve per coedge")
        use = coedge["pcurves"][0]
        pcurve = tables["carriers"][use["pcurve_id"]]
        require(pcurve["domain"] == "pcurve", "pcurve domain mismatch")
        state = pcurve.get("pcurve_state") or {}
        require(
            state.get("wrapper_reversed") in (None, False),
            "reversed pcurve wrappers are unsupported",
        )
        require(
            pcurve.get("provenance", {}).get("tag")
            == "derived_nurbs_isoparametric_pcurve",
            "only derived isoparametric pcurve parameterization is verified",
        )
        ranges = {
            "edge_parameter_range": edge.get("parameter_range"),
            "use_curve_parameter_range": coedge.get("use_curve_parameter_range"),
            "pcurve_use_parameter_range": use.get("parameter_range"),
            "pcurve_state_parameter_range": state.get("parameter_range"),
        }
        require(
            all(value is None for value in ranges.values()),
            "source range semantics are outside this derived-interval profile",
        )
        uv_evaluate = line(pcurve["definition"], "uv")
        edges.append(
            {
                "id": identity,
                "edge_id": edge["id"],
                "vertex_ids": vertex_ids,
                "endpoints": endpoints,
                "interval": interval,
                "axis": axis,
                "evaluate": evaluate,
                "uv_evaluate": uv_evaluate,
                "uv_endpoints": [uv_evaluate(t)[0] for t in interval],
                "source_ranges": ranges,
                "pcurve_provenance": pcurve.get("provenance"),
            }
        )
    require(
        len({e["edge_id"] for e in edges}) == 4, "seam/repeated edges are unsupported"
    )
    require(
        all(
            edges[i]["vertex_ids"][1] == edges[(i + 1) % 4]["vertex_ids"][0]
            for i in range(4)
        ),
        "vertex loop is not closed",
    )
    rectangle = rectangle_check(
        [e["uv_endpoints"] for e in edges], bounds(surface), uv_tolerance
    )
    return {"surface": surface, "edges": edges, "rectangle": rectangle, "loop": loop}


def step_faces(path: Path, version: str, indices: set) -> dict:
    import OCP
    from OCP.BRepAdaptor import (
        BRepAdaptor_Curve,
        BRepAdaptor_Curve2d,
        BRepAdaptor_Surface,
    )
    from OCP.BRepTools import BRepTools, BRepTools_WireExplorer
    from OCP.gp import gp_Pnt, gp_Pnt2d, gp_Vec, gp_Vec2d
    from OCP.IFSelect import IFSelect_RetDone
    from OCP.STEPControl import STEPControl_Reader
    from OCP.TopAbs import (
        TopAbs_EDGE,
        TopAbs_FACE,
        TopAbs_FORWARD,
        TopAbs_REVERSED,
        TopAbs_WIRE,
    )
    from OCP.TopoDS import TopoDS

    require(
        OCP.__version__ == version, "OCP version differs from pinned indexing contract"
    )
    reader = STEPControl_Reader()
    require(reader.ReadFile(str(path)) == IFSelect_RetDone, "STEP reader failed")
    reader.SetSystemLengthUnit(1.0)
    require(
        reader.TransferRoots() > 0 and not reader.OneShape().IsNull(),
        "STEP transfer failed",
    )
    shape = reader.OneShape()
    all_edges = _subshapes(shape, TopAbs_EDGE)
    all_faces = _subshapes(shape, TopAbs_FACE)
    require(all(1 <= i <= len(all_faces) for i in indices), "missing STEP face")

    def orientation(shape):
        require(
            shape.Orientation() in (TopAbs_FORWARD, TopAbs_REVERSED),
            "unsupported STEP orientation",
        )
        return "forward" if shape.Orientation() == TopAbs_FORWARD else "reversed"

    def evaluator(adaptor, point_type, vector_type):
        def evaluate(t):
            point, derivative = point_type(), vector_type()
            adaptor.D1(t, point, derivative)
            return tuple(map(finite, point.Coord())), tuple(
                map(finite, derivative.Coord())
            )

        return evaluate

    result = {}
    for index in sorted(indices):
        face = TopoDS.Face_s(all_faces[index - 1])
        surface = BRepAdaptor_Surface(face)
        require(
            surface.GetType().name == "GeomAbs_BSplineSurface", "STEP face is not NURBS"
        )
        wires = _subshapes(face, TopAbs_WIRE)
        require(
            len(wires) == 1 and wires[0].IsSame(BRepTools.OuterWire_s(face)),
            "expected one outer STEP wire",
        )
        wire = TopoDS.Wire_s(wires[0])
        explorer = BRepTools_WireExplorer(wire, face)
        edges = []
        while explorer.More():
            require(len(edges) < 4, "STEP wire exceeds four edges")
            edge = explorer.Current()
            refs = [i for i, base in enumerate(all_edges, 1) if base.IsSame(edge)]
            require(len(refs) == 1, "ambiguous STEP edge index")
            curve, uv = BRepAdaptor_Curve(edge), BRepAdaptor_Curve2d(edge, face)
            if uv.GetType().name == "GeomAbs_BSplineCurve":
                spline = uv.BSpline()
                require(
                    spline.Degree() == 1
                    and spline.NbPoles() == 2
                    and spline.NbKnots() == 2
                    and not spline.IsPeriodic()
                    and spline.Multiplicity(1) == spline.Multiplicity(2) == 2
                    and spline.Weight(1) == spline.Weight(2) > 0,
                    "STEP pcurve is not an affine single-span line",
                )
            else:
                require(
                    uv.GetType().name == "GeomAbs_Line", "STEP pcurve is not linear"
                )
            interval = [finite(curve.FirstParameter()), finite(curve.LastParameter())]
            require(interval[0] < interval[1], "invalid STEP edge interval")
            require(
                interval == [uv.FirstParameter(), uv.LastParameter()],
                "STEP curve/pcurve ranges differ",
            )
            if orientation(edge) == "reversed":
                interval.reverse()
            evaluate = evaluator(curve, gp_Pnt, gp_Vec)
            uv_evaluate = evaluator(uv, gp_Pnt2d, gp_Vec2d)
            edges.append(
                {
                    "index": refs[0],
                    "interval": interval,
                    "evaluate": evaluate,
                    "uv_evaluate": uv_evaluate,
                    "endpoints": [evaluate(t)[0] for t in interval],
                    "uv_endpoints": [uv_evaluate(t)[0] for t in interval],
                }
            )
            explorer.Next()
        require(
            len(edges) == len(_subshapes(wire, TopAbs_EDGE)) == 4
            and len({e["index"] for e in edges}) == 4,
            "incomplete STEP wire exploration",
        )
        result[index] = {
            "sense": orientation(face),
            "edges": edges,
            "surface": surface.BSpline(),
        }
    return result


def compare_edge(
    native: dict,
    reference: dict,
    surface: Nurbs,
    reference_surface,
    tolerances: dict,
    uv_tolerance: float,
    subdivisions: int,
) -> dict:
    relation = endpoint_relation(
        native["endpoints"], reference["endpoints"], tolerances["position_mm"]
    )
    a, b = native["interval"]
    c, d = reference["interval"]
    if relation == "reversed":
        c, d = d, c  # Diagnostic alignment only; orientation remains a failure.
    fractions = [0.0, 1.0]
    if native["axis"] is not None:
        fractions = sorted(
            (t - a) / (b - a) for t in native["axis"].samples(subdivisions)
        )
    varying = [
        i
        for i, (x, y) in enumerate(zip(*native["uv_endpoints"], strict=True))
        if abs(x - y) > uv_tolerance
    ]
    require(len(varying) == 1, "expected an isoparametric pcurve")
    # Straight boundary curves still sample every knot of the varying surface axis.
    axis = varying[0]
    u0, u1 = [uv[axis] for uv in native["uv_endpoints"]]
    fractions = sorted(
        set(fractions)
        | {
            min(1.0, max(0.0, (u - u0) / (u1 - u0)))
            for u in surface.axes[axis].samples(subdivisions)
        }
    )
    require(len(fractions) <= 100_000, "boundary sample budget exceeded")
    errors = {
        key: []
        for key in (
            "native_lift_mm",
            "step_curve_mm",
            "step_lift_mm",
            "pcurve_uv",
            "tangent_ratio",
        )
    }
    samples = []
    for fraction in fractions:
        t, r = a + (b - a) * fraction, c + (d - c) * fraction
        point, tangent = native["evaluate"](t)
        uv, uv_tangent = native["uv_evaluate"](t)
        lifted, lifted_tangent = lift(surface, uv, uv_tangent, uv_tolerance)
        step_point, step_tangent = reference["evaluate"](r)
        step_uv, step_uv_tangent = reference["uv_evaluate"](r)
        step_lift, (du, dv) = support.step_evaluate(reference_surface, step_uv)
        step_lift_tangent = tuple(
            x * step_uv_tangent[0] + y * step_uv_tangent[1]
            for x, y in zip(du, dv, strict=True)
        )
        native_tangent = scale(tangent, b - a)
        other_tangents = [
            scale(lifted_tangent, b - a),
            scale(step_tangent, d - c),
            scale(step_lift_tangent, d - c),
        ]
        derivative_errors = [
            finite(math.dist(native_tangent, other)) for other in other_tangents
        ]
        derivative_ratios = [
            error
            / (
                tolerances["derivative_absolute_mm"]
                + tolerances["derivative_relative"]
                * max(math.hypot(*native_tangent), math.hypot(*other))
            )
            for error, other in zip(derivative_errors, other_tangents, strict=True)
        ]
        measured = {
            "native_lift_mm": math.dist(point, lifted),
            "step_curve_mm": math.dist(point, step_point),
            "step_lift_mm": math.dist(step_point, step_lift),
            "pcurve_uv": math.dist(uv, step_uv),
            "tangent_ratio": max(derivative_ratios),
        }
        for key, value in measured.items():
            errors[key].append(finite(value))
        samples.append(
            {
                "fraction": fraction,
                "native_parameter": t,
                "aligned_step_parameter": r,
                "native_point_mm": point,
                "native_uv": uv,
                "step_point_mm": step_point,
                "step_uv": step_uv,
                "native_tangent_mm_per_fraction": native_tangent,
                "aligned_step_tangent_mm_per_fraction": other_tangents[1],
                "maximum_tangent_error_mm_per_fraction": max(derivative_errors),
                "errors": measured,
            }
        )
    checks = {
        key: support.metric(
            values,
            1.0
            if key == "tangent_ratio"
            else uv_tolerance
            if key == "pcurve_uv"
            else tolerances["position_mm"],
        )
        for key, values in errors.items()
    }
    return {
        "native_coedge_id": native["id"],
        "step_edge_index": reference["index"],
        "traversal_relation": relation,
        "derived_directed_parameter_range": native["interval"],
        "parameter_range_basis": (
            "DERIVED from native vertices; not source trim metadata"
        ),
        "step_directed_parameter_range": reference["interval"],
        "source_ranges": native["source_ranges"],
        "pcurve_provenance": native["pcurve_provenance"],
        "checks": checks,
        "derived_boundary_gate_passed": all(c["passed"] for c in checks.values()),
        "oriented_edge_gate_passed": relation == "same",
        "sample_count": len(samples),
        "samples": samples,
    }


def cyclic_equal(left: list, right: list) -> bool:
    return len(left) == len(right) and any(
        left == right[i:] + right[:i] for i in range(len(right))
    )


def run(oracle_path: Path, root: Path) -> dict:
    oracle = json.loads(oracle_path.read_text())
    Draft202012Validator(
        json.loads((ROOT / "docs/schemas/nurbs-trim-oracle.schema.json").read_text())
    ).validate(oracle)
    uv_tolerance = finite(oracle["uv_tolerance"])
    require(uv_tolerance > 0, "UV tolerance must be positive")
    support_path = support.checked_file(oracle_path.parent, oracle["support_oracle"])
    support_report = support.run(support_path, root)
    require(support_report["passed"], "support geometry comparison failed")
    pinned = json.loads(support_path.read_text())
    native_path, step_path = (
        support.checked_file(root, pinned[key]) for key in ("native", "neutral")
    )
    geometry = sldkit.decode_geometry_file(native_path).to_dict()["geometry"]
    require(
        geometry is not None
        and geometry["source"]["sha256"] == pinned["native"]["sha256"]
        and geometry["source"]["byte_len"] == pinned["native"]["byte_size"]
        and geometry["length_unit"] == "millimeter",
        "decoded source identity/unit mismatch",
    )
    faces = selected_faces(geometry, pinned["configuration"])
    bindings = support.unique(oracle["face_bindings"], "native_face_id")
    require(
        bool(faces) and set(faces) == set(bindings),
        "bindings must cover selected NURBS faces exactly once",
    )
    surface_bindings = {
        b["native_id"]: b["step_subshape_index"]
        for b in pinned["bindings"]
        if b["domain"] == "surface"
    }
    indices = {b["step_face_index"] for b in bindings.values()}
    require(len(indices) == len(bindings), "duplicate STEP face binding")
    references = step_faces(step_path, pinned["reference_tool_version"], indices)
    comparisons = []
    for identity, binding in bindings.items():
        face = faces[identity]
        require(
            surface_bindings.get(face["surface_id"]) == binding["step_face_index"],
            "trim face differs from pinned support correspondence",
        )
        native = native_boundary(
            geometry["model"], face, pinned["tolerances"]["position_mm"], uv_tolerance
        )
        reference = references[binding["step_face_index"]]
        edge_bindings = support.unique(binding["coedges"], "native_coedge_id")
        require(
            set(edge_bindings) == {e["id"] for e in native["edges"]},
            "incomplete native coedge correspondence",
        )
        step_edges = {e["index"]: e for e in reference["edges"]}
        require(
            len({b["step_edge_index"] for b in edge_bindings.values()}) == 4
            and {b["step_edge_index"] for b in edge_bindings.values()}
            == set(step_edges),
            "incomplete or duplicate STEP edge correspondence",
        )
        step_rectangle = rectangle_check(
            [e["uv_endpoints"] for e in reference["edges"]],
            bounds(native["surface"]),
            uv_tolerance,
        )
        step_xyz_closure = support.metric(
            [
                math.dist(
                    reference["edges"][i]["endpoints"][1],
                    reference["edges"][(i + 1) % 4]["endpoints"][0],
                )
                for i in range(4)
            ],
            pinned["tolerances"]["position_mm"],
        )
        compared = [
            compare_edge(
                edge,
                step_edges[edge_bindings[edge["id"]]["step_edge_index"]],
                native["surface"],
                reference["surface"],
                pinned["tolerances"],
                uv_tolerance,
                pinned["samples_per_knot_span"],
            )
            for edge in native["edges"]
        ]
        native_order = [
            edge_bindings[e["id"]]["step_edge_index"] for e in native["edges"]
        ]
        step_order = [e["index"] for e in reference["edges"]]
        orientation = {
            "native_face_sense": face["sense"],
            "step_face_sense": reference["sense"],
            "face_sense_matches": face["sense"] == reference["sense"],
            "native_order_as_step_edge_indices": native_order,
            "step_wire_order": step_order,
            "cyclic_order_matches": cyclic_equal(native_order, step_order),
            "all_edge_traversals_match": all(
                c["oriented_edge_gate_passed"] for c in compared
            ),
        }
        comparisons.append(
            {
                "binding": binding,
                "native_rectangle": native["rectangle"],
                "step_rectangle": step_rectangle,
                "step_xyz_closure_mm": step_xyz_closure,
                "source_loop_boundary_role": native["loop"]["boundary_role"],
                "orientation": orientation,
                "coedges": compared,
                "derived_boundary_gate_passed": step_xyz_closure["passed"]
                and all(c["derived_boundary_gate_passed"] for c in compared),
                "oriented_trim_gate_passed": all(
                    orientation[k]
                    for k in (
                        "face_sense_matches",
                        "cyclic_order_matches",
                        "all_edge_traversals_match",
                    )
                ),
                "source_trim_gate_passed": False,
            }
        )
    # Recheck pins after both independent captures to avoid mixing changed inputs.
    support.checked_file(oracle_path.parent, oracle["support_oracle"])
    for key in ("native", "neutral"):
        support.checked_file(root, pinned[key])
    gates = {
        key: all(c[key] for c in comparisons)
        for key in (
            "derived_boundary_gate_passed",
            "oriented_trim_gate_passed",
            "source_trim_gate_passed",
        )
    }
    return {
        "schema_version": 1,
        "profile": oracle["profile"],
        "oracle_sha256": support.sha256(oracle_path),
        "support_oracle": oracle["support_oracle"],
        "native": pinned["native"],
        "neutral": pinned["neutral"],
        "configuration": pinned["configuration"],
        "capture": support_report["capture"],
        "decoder": support_report["decoder"],
        "decoder_version": support_report["decoder_version"],
        "tolerances": pinned["tolerances"] | {"uv": uv_tolerance},
        "samples_per_knot_span": pinned["samples_per_knot_span"],
        "support_geometry_gate_passed": True,
        "comparison_completed": True,
        **gates,
        "passed": all(gates.values()),
        "comparisons": comparisons,
        "reference_pcurve_basis": (
            "OCP STEP-imported B-Rep; pcurves may be reconstructed, "
            "not a SolidWorks API capture"
        ),
        "source_trim_limitation": (
            "This profile validates endpoint-derived intervals and derived "
            "isoparametric pcurves only; it cannot certify native source trim metadata."
        ),
        "scope": (
            "Finite boundary point/tangent samples and rectangular UV closure; "
            "no global error bound, holes, seams, periodic or partial NURBS trims, "
            "or oriented normal certification."
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
        report = {
            "schema_version": 1,
            "passed": False,
            "comparison_completed": False,
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
                    "comparison_completed": report["comparison_completed"],
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
