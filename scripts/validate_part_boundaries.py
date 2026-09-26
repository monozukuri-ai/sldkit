#!/usr/bin/env python3
"""Bounded Part boundary validation against pinned SolidWorks API captures.

API checks: every native edge's interval extent/endpoints/midpoint, directed
loop order, planar and bounded cylindrical roles and face areas. Independent Python
evaluators additionally check curve -> pcurve -> surface lifts at 17 points
per coedge, including explicit periodic seams. These latter checks prove
internal consistency, not stored native pcurve or trim metadata.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import runpy
from collections import Counter, defaultdict
from pathlib import Path
from types import SimpleNamespace

import sldkit

ORIENTATION = SimpleNamespace(
    **runpy.run_path(str(Path(__file__).with_name("validate_part_orientation.py")))
)
POSITION_MM = 1e-6
PARAMETER = 1e-9
SAMPLES = 17


def add(a, b):
    return [x + y for x, y in zip(a, b, strict=True)]


def vec(v, dimensions="xyz"):
    return v if isinstance(v, list) else [v[k] for k in dimensions]


def curve(d, t):
    if d["kind"] == "line":
        tangent = vec(d["direction"])
        return add(vec(d["origin"]), ORIENTATION.scale(tangent, t)), tangent
    if d["kind"] in ("circle", "ellipse"):
        direction = vec(
            d["ref_direction"] if d["kind"] == "circle" else d["major_direction"]
        )
        major = d.get("radius", d.get("major_radius"))
        minor = d.get("radius", d.get("minor_radius"))
        x = ORIENTATION.scale(direction, major)
        y = ORIENTATION.scale(ORIENTATION.cross(vec(d["axis"]), direction), minor)
        return (
            add(
                vec(d["center"]),
                add(
                    ORIENTATION.scale(x, math.cos(t)), ORIENTATION.scale(y, math.sin(t))
                ),
            ),
            add(ORIENTATION.scale(x, -math.sin(t)), ORIENTATION.scale(y, math.cos(t))),
        )
    raise ValueError("unsupported curve")


def pcurve(d, t):
    if d["kind"] == "line":
        direction = vec(d["direction"], "uv")
        return add(vec(d["origin"], "uv"), ORIENTATION.scale(direction, t)), direction
    if d["kind"] in ("circle", "ellipse"):
        x = ORIENTATION.scale(
            vec(d["x_axis"], "uv"), d.get("radius", d.get("major_radius"))
        )
        y = ORIENTATION.scale(
            vec(d["y_axis"], "uv"), d.get("radius", d.get("minor_radius"))
        )
        return (
            add(
                vec(d["center"], "uv"),
                add(
                    ORIENTATION.scale(x, math.cos(t)), ORIENTATION.scale(y, math.sin(t))
                ),
            ),
            add(ORIENTATION.scale(x, -math.sin(t)), ORIENTATION.scale(y, math.cos(t))),
        )
    if d["kind"] == "polar_harmonic":
        radial = add(
            vec(d["radial_center"], "uv"),
            add(
                ORIENTATION.scale(vec(d["radial_cos"], "uv"), math.cos(t)),
                ORIENTATION.scale(vec(d["radial_sin"], "uv"), math.sin(t)),
            ),
        )
        derivative = add(
            ORIENTATION.scale(vec(d["radial_cos"], "uv"), -math.sin(t)),
            ORIENTATION.scale(vec(d["radial_sin"], "uv"), math.cos(t)),
        )
        return [
            math.atan2(radial[1], radial[0]),
            d["axial_origin"]
            + d["axial_cos"] * math.cos(t)
            + d["axial_sin"] * math.sin(t),
        ], [
            (radial[0] * derivative[1] - radial[1] * derivative[0])
            / ORIENTATION.dot(radial, radial),
            -d["axial_cos"] * math.sin(t) + d["axial_sin"] * math.cos(t),
        ]
    raise ValueError("unsupported pcurve")


def lift(d, uv, derivative):
    if d["kind"] == "plane":
        x = vec(d["u_axis"])
        y = ORIENTATION.cross(vec(d["normal"]), x)
        return add(
            vec(d["origin"]),
            add(ORIENTATION.scale(x, uv[0]), ORIENTATION.scale(y, uv[1])),
        ), add(ORIENTATION.scale(x, derivative[0]), ORIENTATION.scale(y, derivative[1]))
    if d["kind"] == "cylinder":
        z = vec(d["axis"])
        x = vec(d["ref_direction"])
        y = ORIENTATION.cross(z, x)
        radial = ORIENTATION.scale(
            add(
                ORIENTATION.scale(x, math.cos(uv[0])),
                ORIENTATION.scale(y, math.sin(uv[0])),
            ),
            d["radius"],
        )
        angular = ORIENTATION.scale(
            add(
                ORIENTATION.scale(x, -math.sin(uv[0])),
                ORIENTATION.scale(y, math.cos(uv[0])),
            ),
            d["radius"] * derivative[0],
        )
        return add(vec(d["origin"]), add(radial, ORIENTATION.scale(z, uv[1]))), add(
            angular, ORIENTATION.scale(z, derivative[1])
        )
    raise ValueError("unsupported surface")


def interval(edge):
    value = edge.get("parameter_range")
    if value is None:
        value = (edge.get("derived_parameter_interval") or {}).get("parameter_range")
    if (
        value is None
        or len(value) != 2
        or not all(math.isfinite(v) for v in value)
        or value[0] == value[1]
    ):
        raise ValueError("missing or invalid effective edge interval")
    return value


def cylinder_match(a, b):
    axis_a, axis_b = ORIENTATION.unit(vec(a["axis"])), ORIENTATION.unit(vec(b["axis"]))
    delta = ORIENTATION.sub(vec(b["origin"]), vec(a["origin"]))
    return (
        abs(a["radius"] - b["radius"]) <= POSITION_MM
        and ORIENTATION.norm(ORIENTATION.cross(axis_a, axis_b)) <= PARAMETER
        and ORIENTATION.norm(ORIENTATION.cross(delta, axis_a)) <= POSITION_MM
    )


def step_areas(model, path):
    """Compare cylinder surface groups: STEP may split a native periodic face.

    Match solely by the geometric cylinder (axis line and radius), not area.
    Native faces sharing a cylinder are accumulated; no per-face identity claim.
    """
    import OCP
    from OCP.BRepAdaptor import BRepAdaptor_Surface
    from OCP.BRepGProp import BRepGProp
    from OCP.GProp import GProp_GProps
    from OCP.IFSelect import IFSelect_RetDone
    from OCP.STEPControl import STEPControl_Reader
    from OCP.TopAbs import TopAbs_FACE
    from OCP.TopoDS import TopoDS

    subshapes = runpy.run_path(
        str(Path(__file__).with_name("capture_step_geometry.py"))
    )["_subshapes"]
    reader = STEPControl_Reader()
    if reader.ReadFile(str(path)) != IFSelect_RetDone:
        raise ValueError("STEP reader rejected reference")
    reader.SetSystemLengthUnit(1.0)
    if reader.TransferRoots() <= 0 or reader.OneShape().IsNull():
        raise ValueError("STEP reference transferred no shape")
    carriers = {v["id"]: v["definition"] for v in model["carriers"]}
    loops = {v["id"]: v for v in model["loops"]}
    groups = []
    for face in model["faces"]:
        definition = carriers[face["surface_id"]]
        if definition["kind"] != "cylinder":
            continue
        matches = [g for g in groups if cylinder_match(definition, g["surface"])]
        if len(matches) > 1:
            raise ValueError("ambiguous native cylinder group")
        if not matches:
            groups.append(
                {
                    "surface": definition,
                    "native_face_ids": [],
                    "step_face_indices": [],
                    "native_area_mm2": 0.0,
                    "step_area_mm2": 0.0,
                    "estimated_relative_error": 0.0,
                }
            )
            matches = [groups[-1]]
        group = matches[0]
        group["native_face_ids"].append(face["id"])
        group["native_area_mm2"] += sum(
            loops[i]["derived_boundary_role"]["signed_area_mm2"]
            for i in face["loop_ids"]
        )
    for index, shape in enumerate(subshapes(reader.OneShape(), TopAbs_FACE), 1):
        face = TopoDS.Face_s(shape)
        adaptor = BRepAdaptor_Surface(face)
        if adaptor.GetType().name != "GeomAbs_Cylinder":
            continue
        cylinder = adaptor.Cylinder()
        definition = {
            "origin": list(cylinder.Location().Coord()),
            "axis": list(cylinder.Axis().Direction().Coord()),
            "radius": cylinder.Radius(),
        }
        matches = [g for g in groups if cylinder_match(definition, g["surface"])]
        if len(matches) != 1:
            raise ValueError(f"STEP face {index}: ambiguous or unmatched cylinder")
        properties = GProp_GProps()
        error = BRepGProp.SurfaceProperties_s(face, properties, 1e-10, False)
        if not math.isfinite(error) or error > 1e-9:
            raise ValueError("STEP face integration did not converge")
        group = matches[0]
        group["step_face_indices"].append(index)
        group["step_area_mm2"] += properties.Mass()
        group["estimated_relative_error"] = max(
            group["estimated_relative_error"], error
        )
    if not groups or any(not g["step_face_indices"] for g in groups):
        raise ValueError("incomplete STEP cylinder coverage")
    for group in groups:
        group["error_mm2"] = abs(group["native_area_mm2"] - group["step_area_mm2"])
        group["passed"] = (
            group["error_mm2"] <= 1e-6 + abs(group["step_area_mm2"]) * 1e-8
        )
    return {
        "ocp_version": str(OCP.__version__),
        "groups": groups,
        "passed": all(g["passed"] for g in groups),
    }


def validate(model, orientation, rows, independent_areas=None):
    if not orientation["passed"]:
        raise ValueError("orientation precondition failed")
    tables = {
        kind: {item["id"]: item for item in model[kind]}
        for kind in (
            "faces",
            "loops",
            "coedges",
            "edges",
            "vertices",
            "points",
            "carriers",
        )
    }
    api_edges = {tuple(map(int, r[1:5])): r for r in rows if r[0] == "COEDGE"}
    api_loops = {tuple(map(int, r[1:4])): r for r in rows if r[0] == "LOOP"}
    api_faces = {tuple(map(int, r[1:3])): r for r in rows if r[0] == "FACE"}
    bindings = {
        c["coedge_id"]: tuple(c["api_id"])
        for f in orientation["correspondences"]
        for c in f["coedges"]
    }
    api_face_bindings = {
        f["native_face_id"]: tuple(f["api_face_id"])
        for f in orientation["correspondences"]
    }

    def vertex(identity):
        return tables["points"][tables["vertices"][identity]["point_id"]]["position"]

    edge_checks = {}
    loop_checks = []
    pcurve_checks = []
    for identity, api_id in bindings.items():
        coedge = tables["coedges"][identity]
        edge = tables["edges"][coedge["edge_id"]]
        definition = tables["carriers"][edge["curve_id"]]["definition"]
        bounds = interval(edge)
        row = api_edges[api_id]
        expected_span = abs(float(row[13]) - float(row[12])) * (
            1000 if definition["kind"] == "line" else 1
        )
        span_error = abs(abs(bounds[1] - bounds[0]) - expected_span)
        start = curve(definition, bounds[0])[0]
        end = curve(definition, bounds[1])[0]
        endpoint_error = max(
            ORIENTATION.distance(start, vertex(edge["start_vertex_id"])),
            ORIENTATION.distance(end, vertex(edge["end_vertex_id"])),
        )
        closed = ORIENTATION.distance(start, end) <= POSITION_MM
        midpoint_error = (
            None
            if closed
            else ORIENTATION.distance(
                curve(definition, sum(bounds) / 2)[0],
                ORIENTATION.scale(ORIENTATION.numbers(row[17:20]), 1000),
            )
        )
        passed = (
            span_error <= (POSITION_MM if definition["kind"] == "line" else PARAMETER)
            and endpoint_error <= POSITION_MM
            and (midpoint_error is None or midpoint_error <= POSITION_MM)
        )
        check = {
            "edge_id": edge["id"],
            "span_error": span_error,
            "endpoint_error_mm": endpoint_error,
            "midpoint_error_mm": midpoint_error,
            "closed_seam_gauge": closed,
            "passed": passed,
        }
        previous = edge_checks.get(edge["id"])
        if previous is not None and previous["passed"] != passed:
            raise ValueError("API edge uses disagree")
        edge_checks[edge["id"]] = check
    matched_api_loops = set()
    for loop in model["loops"]:
        face = tables["faces"][loop["face_id"]]
        surface = tables["carriers"][face["surface_id"]]["definition"]
        native_order = defaultdict(list)
        ends = []
        for index, identity in enumerate(loop["coedge_ids"]):
            coedge = tables["coedges"][identity]
            if (
                coedge["next_id"]
                != loop["coedge_ids"][(index + 1) % len(loop["coedge_ids"])]
                or coedge["previous_id"]
                != loop["coedge_ids"][(index - 1) % len(loop["coedge_ids"])]
            ):
                raise ValueError("broken public ring order")
            if identity in bindings:
                api_id = bindings[identity]
                native_order[api_id[:3]].append(api_id[3])
            edge = tables["edges"][coedge["edge_id"]]
            definition = tables["carriers"][edge["curve_id"]]["definition"]
            bounds = list(interval(edge))
            if coedge["sense"] == "reversed":
                bounds.reverse()
            ends.append([curve(definition, t)[0] for t in bounds])
            if len(coedge["pcurves"]) != 1:
                raise ValueError("expected one pcurve per coedge")
            pc = tables["carriers"][coedge["pcurves"][0]["pcurve_id"]]["definition"]
            errors = []
            tangents = []
            for i in range(SAMPLES):
                t = bounds[0] + (bounds[1] - bounds[0]) * i / (SAMPLES - 1)
                p, tangent = curve(definition, t)
                uv, differential = pcurve(pc, t)
                lifted, lifted_tangent = lift(surface, uv, differential)
                errors.append(ORIENTATION.distance(p, lifted))
                tangents.append(
                    ORIENTATION.dot(
                        ORIENTATION.unit(tangent), ORIENTATION.unit(lifted_tangent)
                    )
                )
            pcurve_checks.append(
                {
                    "coedge_id": identity,
                    "max_lift_error_mm": max(errors),
                    "minimum_tangent_dot": min(tangents),
                    "passed": max(errors) <= POSITION_MM
                    and all(abs(v - 1) <= PARAMETER for v in tangents),
                }
            )
        for api_id, order in native_order.items():
            if (
                api_id in matched_api_loops
                or api_id[:2] != api_face_bindings[face["id"]]
            ):
                raise ValueError("ambiguous loop membership")
            matched_api_loops.add(api_id)
            count = int(api_loops[api_id][5])
            if (
                len(order) != count
                or len(set(order)) != count
                or any(
                    order[(i + 1) % count] != (v + 1) % count
                    for i, v in enumerate(order)
                )
            ):
                raise ValueError("native/API directed loop order differs")
        closure = max(
            ORIENTATION.distance(pair[1], ends[(i + 1) % len(ends)][0])
            for i, pair in enumerate(ends)
        )
        role = loop.get("derived_boundary_role")
        role_passed = None
        if surface["kind"] in ("plane", "cylinder"):
            if role is None or not native_order:
                raise ValueError("missing analytic role or native loop")
            if surface["kind"] == "plane" and len(native_order) != 1:
                raise ValueError("ambiguous planar loop")
            api_roles = {
                "outer" if api_loops[identity][4] == "-1" else "inner"
                for identity in native_order
            }
            role_passed = (
                api_roles == {role["role"]} and loop["boundary_role"] == "unspecified"
            )
        loop_checks.append(
            {
                "loop_id": loop["id"],
                "api_loops": [list(k) for k in native_order],
                "closure_error_mm": closure,
                "surface_kind": surface["kind"],
                "role_passed": role_passed,
                "passed": closure <= POSITION_MM and role_passed is not False,
            }
        )
    if matched_api_loops != set(api_loops):
        raise ValueError("incomplete API loop coverage")
    areas = []
    for face in model["faces"]:
        kind = tables["carriers"][face["surface_id"]]["kind"]
        if kind not in ("plane", "cylinder"):
            continue
        area = sum(
            tables["loops"][identity]["derived_boundary_role"]["signed_area_mm2"]
            for identity in face["loop_ids"]
        )
        api_area = float(api_faces[api_face_bindings[face["id"]]][4]) * 1e6
        reference = api_area
        error = abs(area - reference)
        areas.append(
            {
                "face_id": face["id"],
                "surface_kind": kind,
                "error_mm2": error,
                "api_approximate_area_error_mm2": abs(area - api_area),
                "reference": "SolidWorks GetArea (approximate)",
                "passed": error <= 1e-6 + abs(reference) * 1e-8
                if kind == "plane"
                else None,
            }
        )
    checks = [*edge_checks.values(), *loop_checks, *pcurve_checks, *areas]
    return {
        "passed": all(c["passed"] is not False for c in checks)
        and (
            not any(c["surface_kind"] == "cylinder" for c in areas)
            or (independent_areas is not None and independent_areas["passed"])
        ),
        "cylindrical_area_verified": independent_areas is not None
        and independent_areas["passed"],
        "native_edges": len(edge_checks),
        "api_loops": len(matched_api_loops),
        "public_loops": len(loop_checks),
        **{
            label + "_roles": dict(
                Counter(
                    loop["derived_boundary_role"]["role"]
                    for loop in model["loops"]
                    if loop.get("derived_boundary_role")
                    and tables["carriers"][
                        tables["faces"][loop["face_id"]]["surface_id"]
                    ]["kind"]
                    == kind
                )
            )
            for label, kind in (("planar", "plane"), ("cylindrical", "cylinder"))
        },
        "pcurve_coedges": len(pcurve_checks),
        "pcurve_samples": len(pcurve_checks) * SAMPLES,
        "max_lift_error_mm": max(c["max_lift_error_mm"] for c in pcurve_checks),
        "max_planar_area_error_mm2": max(
            (c["error_mm2"] for c in areas if c["surface_kind"] == "plane"),
            default=None,
        ),
        "max_cylindrical_api_approximate_area_error_mm2": max(
            (c["error_mm2"] for c in areas if c["surface_kind"] == "cylinder"),
            default=None,
        ),
        "max_cylindrical_step_group_area_error_mm2": (
            max(g["error_mm2"] for g in independent_areas["groups"])
            if independent_areas is not None
            else None
        ),
        "cylindrical_loop_roles_verified": any(
            c["surface_kind"] == "cylinder" for c in loop_checks
        )
        and all(
            c["role_passed"] is True
            for c in loop_checks
            if c["surface_kind"] == "cylinder"
        ),
        "source_trim_verified": False,
        "periodic_loop_roles_verified": False,
        "edges": list(edge_checks.values()),
        "loops": loop_checks,
        "pcurves": pcurve_checks,
        "planar_face_areas": [c for c in areas if c["surface_kind"] == "plane"],
        "cylindrical_face_areas": [c for c in areas if c["surface_kind"] == "cylinder"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--capture-sha256", required=True)
    parser.add_argument("--step", type=Path)
    parser.add_argument("--step-sha256")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if bool(args.step) != bool(args.step_sha256):
        parser.error("--step and --step-sha256 must be supplied together")
    if args.output.resolve() in tuple(
        p.resolve() for p in (args.source, args.capture, args.step) if p is not None
    ):
        parser.error("output must not overwrite evidence")
    report = {
        "schema_version": 2,
        "passed": False,
        "position_tolerance_mm": POSITION_MM,
        "parameter_tolerance": PARAMETER,
    }
    try:
        for name in ("source", "capture", *(["step"] if args.step else [])):
            path = getattr(args, name)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            if digest != getattr(args, name + "_sha256"):
                raise ValueError(name + " SHA-256 mismatch")
            report[name] = {
                "name": path.name,
                "sha256": digest,
                "bytes": path.stat().st_size,
            }
        faces, groups, edges, _ = ORIENTATION.read_capture(args.capture, args.source)
        model = sldkit.decode_geometry_file(args.source).to_dict()["geometry"]["model"]
        orientation = ORIENTATION.compare(model, faces, groups, edges)
        independent_areas = None
        if args.step:
            independent_areas = step_areas(model, args.step)
            report["step_area_reference"] = independent_areas
        with args.capture.open(encoding="ascii", newline="") as stream:
            rows = list(csv.reader(stream))
        report.update(validate(model, orientation, rows, independent_areas))
    except (ValueError, KeyError, IndexError, ZeroDivisionError, OSError) as error:
        report.update(passed=False, error=str(error))
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(
        json.dumps(
            {
                k: v
                for k, v in report.items()
                if k
                not in (
                    "edges",
                    "loops",
                    "pcurves",
                    "planar_face_areas",
                    "cylindrical_face_areas",
                    "step_area_reference",
                )
            }
        )
    )
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
