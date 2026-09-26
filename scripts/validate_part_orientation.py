#!/usr/bin/env python3
"""Compare a pinned Part with capture_part_orientation.swb output.

Bounded to one solid with plane/cylinder faces and line/circle/ellipse edges.
Checks all native coedges at API midpoints, excluding explicitly derived seams.
This is an orientation/support check, not a stored trim or loop-role proof.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
from collections import defaultdict
from pathlib import Path

import sldkit

POSITION_TOLERANCE_MM = 1e-6
DIRECTION_TOLERANCE = 1e-9


def vector(value):
    return value if isinstance(value, list) else [value[key] for key in "xyz"]


def sub(a, b):
    return [x - y for x, y in zip(a, b, strict=True)]


def dot(a, b):
    return sum(x * y for x, y in zip(a, b, strict=True))


def scale(a, factor):
    return [x * factor for x in a]


def norm(a):
    return math.sqrt(dot(a, a))


def unit(a):
    length = norm(a)
    if not math.isfinite(length) or length <= 0:
        raise ValueError("nonfinite or zero direction")
    return scale(a, 1 / length)


def cross(a, b):
    return [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]


def distance(a, b):
    return norm(sub(a, b))


def numbers(values):
    result = list(map(float, values))
    if not all(math.isfinite(value) for value in result):
        raise ValueError("nonfinite API sample")
    return result


def sense(value):
    if value not in ("forward", "reversed"):
        raise ValueError(f"unsupported sense: {value}")
    return 1 if value == "forward" else -1


def read_capture(path, source):
    with path.open(encoding="ascii", newline="") as stream:
        rows = list(csv.reader(stream))
    if not rows or rows[0][:2] != ["CAPTURE", "1"] or rows[-1] != ["COMPLETE"]:
        raise ValueError("incomplete or unknown API capture")
    if [r for r in rows if r[0] == "SOURCE"] != [
        ["SOURCE", source.name, str(source.stat().st_size)]
    ] or [r for r in rows if r[0] == "UNITS"] != [["UNITS", "meter"]]:
        raise ValueError("API source identity or units mismatch")
    bodies = [r for r in rows if r[0] == "BODY"]
    if len(bodies) != 1 or bodies[0][1:3] != ["0", "0"]:
        raise ValueError("only one solid body is qualified")
    faces, loops, coedges = {}, {}, {}
    groups = defaultdict(list)
    for row in rows:
        if row[0] == "FACE":
            key = tuple(map(int, row[1:3]))
            if key in faces or row[3] not in ("0", "-1"):
                raise ValueError("duplicate face or invalid API face sense")
            if row[5] not in ("plane", "cylinder"):
                raise ValueError("unsupported API surface")
            faces[key] = {"opposite": row[3] == "-1", "kind": row[5]}
        elif row[0] == "LOOP":
            key = tuple(map(int, row[1:4]))
            if key in loops:
                raise ValueError("duplicate API loop")
            loops[key] = int(row[5])
        elif row[0] == "COEDGE":
            if len(row) != 35:
                raise ValueError("unknown API coedge array layout")
            key = tuple(map(int, row[1:5]))
            if key in coedges:
                raise ValueError("duplicate API coedge")
            coedges[key] = {
                "id": list(key),
                "a": scale(numbers(row[6:9]), 1000),
                "b": scale(numbers(row[9:12]), 1000),
                "p": scale(numbers(row[17:20]), 1000),
                "t": numbers(row[20:23]),
                "n": numbers(row[24:27]),
            }
            # Packed/reserved array elements are deliberately uninterpreted.
            groups[key[:2]].append(coedges[key])
    if len(faces) != int(bodies[0][3]) or set(groups) != set(faces):
        raise ValueError("incomplete API face coverage")
    if any(key[:2] not in faces for key in loops):
        raise ValueError("orphan API loop")
    if any(key[:3] not in loops for key in coedges):
        raise ValueError("orphan API coedge")
    if any(sum(k[:3] == key for k in coedges) != n for key, n in loops.items()):
        raise ValueError("incomplete API loop coverage")
    return faces, groups, int(bodies[0][4]), len(loops)


def support_match(sample, coedge, tables):
    edge = tables["edges"][coedge["edge_id"]]
    definition = tables["carriers"][edge["curve_id"]]["definition"]

    def point(vertex_id):
        point_id = tables["vertices"][vertex_id]["point_id"]
        return vector(tables["points"][point_id]["position"])

    pa, pb = point(edge["start_vertex_id"]), point(edge["end_vertex_id"])
    a, b = sample["a"], sample["b"]
    if (
        distance(a, b) > POSITION_TOLERANCE_MM
        and min(
            max(distance(pa, a), distance(pb, b)),
            max(distance(pa, b), distance(pb, a)),
        )
        > POSITION_TOLERANCE_MM
    ):
        return None
    kind = definition["kind"]
    if kind == "line":
        tangent = unit(vector(definition["direction"]))
        delta = sub(sample["p"], vector(definition["origin"]))
        error = norm(sub(delta, scale(tangent, dot(delta, tangent))))
    elif kind in ("circle", "ellipse"):
        delta = sub(sample["p"], vector(definition["center"]))
        axis = unit(vector(definition["axis"]))
        error = abs(dot(delta, axis))
        if kind == "circle":
            error = max(error, abs(norm(delta) - definition["radius"]))
            tangent = unit(cross(axis, delta))
        else:
            x = unit(vector(definition["major_direction"]))
            y = cross(axis, x)
            major, minor = definition["major_radius"], definition["minor_radius"]
            cosine, sine = dot(delta, x) / major, dot(delta, y) / minor
            error = max(error, abs(cosine * cosine + sine * sine - 1) * major)
            tangent = unit(sub(scale(y, minor * cosine), scale(x, major * sine)))
    else:
        raise ValueError(f"unsupported curve: {kind}")
    if not math.isfinite(error) or error > POSITION_TOLERANCE_MM:
        return None
    tangent = scale(tangent, sense(coedge["sense"]))
    return {
        "coedge_id": coedge["id"],
        "tangent_dot": dot(tangent, unit(sample["t"])),
        "support_error_mm": error,
    }


def normal(face, point, tables):
    definition = tables["carriers"][face["surface_id"]]["definition"]
    if definition["kind"] == "plane":
        result = unit(vector(definition["normal"]))
        error = abs(dot(sub(point, vector(definition["origin"])), result))
    elif definition["kind"] == "cylinder":
        axis = unit(vector(definition["axis"]))
        delta = sub(point, vector(definition["origin"]))
        radial = sub(delta, scale(axis, dot(delta, axis)))
        error = abs(norm(radial) - definition["radius"])
        result = unit(radial)
    else:
        raise ValueError("unsupported surface")
    return scale(result, sense(face["sense"])), error


def compare(model, api_faces, groups, edge_count):
    tables = {}
    for kind in (
        "faces",
        "loops",
        "coedges",
        "edges",
        "vertices",
        "points",
        "carriers",
    ):
        tables[kind] = {item["id"]: item for item in model[kind]}
        if len(tables[kind]) != len(model[kind]):
            raise ValueError(f"duplicate native IDs: {kind}")
    if len(model["bodies"]) != 1 or model["bodies"][0]["kind"] != "solid":
        raise ValueError("native body count/kind mismatch")
    for kind in ("bodies", "regions", "shells"):
        if not model[kind] or any(
            item["provenance"]["exactness"] != "byte_exact" for item in model[kind]
        ):
            raise ValueError(f"source hierarchy unavailable: {kind}")
    results = []
    for key, api_face in api_faces.items():
        candidates = []
        for face in model["faces"]:
            if tables["carriers"][face["surface_id"]]["kind"] != api_face["kind"]:
                continue
            coedges = [
                tables["coedges"][identity]
                for loop in face["loop_ids"]
                for identity in tables["loops"][loop]["coedge_ids"]
                if tables["coedges"][identity]["provenance"]["tag"] == "00_11"
            ]
            if len(coedges) != len(groups[key]):
                continue
            matches = []
            for sample in groups[key]:
                options = [
                    match
                    for coedge in coedges
                    if (match := support_match(sample, coedge, tables)) is not None
                ]
                if len(options) != 1:
                    break
                match = options[0]
                outward = scale(sample["n"], -1 if api_face["opposite"] else 1)
                native_normal, surface_error = normal(face, sample["p"], tables)
                if (
                    not math.isfinite(surface_error)
                    or surface_error > POSITION_TOLERANCE_MM
                ):
                    break
                match.update(
                    api_id=sample["id"],
                    normal_dot=dot(native_normal, unit(outward)),
                    surface_error_mm=surface_error,
                )
                matches.append(match)
            if len(matches) == len(coedges) and len(
                {m["coedge_id"] for m in matches}
            ) == len(matches):
                candidates.append(
                    {
                        "native_face_id": face["id"],
                        "api_face_id": list(key),
                        "coedges": matches,
                    }
                )
        if len(candidates) != 1:
            raise ValueError(f"face correspondence is not unique: {key}")
        results.extend(candidates)
    matched_faces = {face["native_face_id"] for face in results}
    samples = [sample for face in results for sample in face["coedges"]]
    matched_coedges = {sample["coedge_id"] for sample in samples}
    native_coedges = {
        c["id"] for c in model["coedges"] if c["provenance"]["tag"] == "00_11"
    }
    matched_edges = {tables["coedges"][c]["edge_id"] for c in matched_coedges}
    if matched_faces != set(tables["faces"]) or len(matched_faces) != len(results):
        raise ValueError("face coverage is not bijective")
    if matched_coedges != native_coedges or len(matched_coedges) != len(samples):
        raise ValueError("coedge coverage is not bijective")
    if len(matched_edges) != edge_count or not samples:
        raise ValueError("edge coverage mismatch")
    normals = all(abs(s["normal_dot"] - 1) <= DIRECTION_TOLERANCE for s in samples)
    tangents = all(abs(s["tangent_dot"] - 1) <= DIRECTION_TOLERANCE for s in samples)
    return {
        "passed": normals and tangents,
        "face_normals_passed": normals,
        "coedge_tangents_passed": tangents,
        "matched_faces": len(results),
        "matched_edges": len(matched_edges),
        "matched_coedges": len(samples),
        "max_support_error_mm": max(s["support_error_mm"] for s in samples),
        "max_surface_error_mm": max(s["surface_error_mm"] for s in samples),
        "minimum_normal_dot": min(s["normal_dot"] for s in samples),
        "minimum_tangent_dot": min(s["tangent_dot"] for s in samples),
        "correspondences": results,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--capture-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.source.resolve(), args.capture.resolve()):
        parser.error("output must not overwrite evidence")
    report = {"schema_version": 1, "passed": False}
    try:
        for kind in ("source", "capture"):
            path = getattr(args, kind)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            if digest != getattr(args, kind + "_sha256"):
                raise ValueError(f"{kind} SHA-256 mismatch")
            report[kind] = {
                "name": path.name,
                "sha256": digest,
                "bytes": path.stat().st_size,
            }
        faces, groups, edges, loops = read_capture(args.capture, args.source)
        decoded = sldkit.decode_geometry_file(args.source).to_dict()
        report.update(compare(decoded["geometry"]["model"], faces, groups, edges))
        report.update(
            api_loops=loops,
            position_tolerance_mm=POSITION_TOLERANCE_MM,
            direction_tolerance=DIRECTION_TOLERANCE,
            source_trim_verified=False,
        )
    except (OSError, ValueError, KeyError, IndexError, ZeroDivisionError) as error:
        report.update(passed=False, error=str(error))
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "correspondences"}))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
