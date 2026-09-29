#!/usr/bin/env python3
"""Compare a pinned saved/reopened NURBS sheet with SolidWorks API samples.

This is a bounded single-body/single-face oracle, not a general API importer.
Curve parameter gauges may differ by an affine map; directed positions and
tangents must agree at every captured sample. Source trim remains a separate gate.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
from pathlib import Path

import sldkit
from nurbs_geometry import Nurbs

TOLERANCE_MM = 1e-6


def distance(a, b):
    return math.dist(a, b)


def read_capture(path, source):
    with path.open(encoding="ascii", newline="") as stream:
        rows = list(csv.reader(stream))
    if (
        len(rows) < 5
        or rows[0][:2] != ["CAPTURE", "2"]
        or rows[1] != ["SOURCE", source.name, str(source.stat().st_size)]
        or rows[2] != ["UNITS", "meter"]
        or rows[-1] != ["COMPLETE"]
    ):
        raise ValueError("capture header/source/completion mismatch")
    edges, samples, supports = {}, {}, {}
    bodies, faces = [], []
    for row in rows[3:-1]:
        if row[0] == "BODY":
            bodies.append(row)
        elif row[0] == "FACE":
            faces.append(row)
        elif row[0] in ("EDGE", "SUPPORT", "SAMPLE"):
            identity = tuple(map(int, row[1:3]))
            if row[0] == "SAMPLE":
                index = int(row[3])
                values = list(map(float, row[4:]))
                if len(values) not in (7, 8) or not all(
                    math.isfinite(v) for v in values
                ):
                    raise ValueError("invalid API sample")
                if index in samples.setdefault(identity, {}):
                    raise ValueError("duplicate API sample")
                # Evaluate2's optional final packed status is retained in the CSV.
                samples[identity][index] = values[:7]
            else:
                table = edges if row[0] == "EDGE" else supports
                if identity in table:
                    raise ValueError("duplicate API edge/support")
                values = list(map(float, row[3:]))
                if not all(math.isfinite(v) for v in values):
                    raise ValueError("nonfinite API data")
                if len(values) != (10 if row[0] == "EDGE" else 5):
                    raise ValueError("invalid API edge/support row")
                table[identity] = values
        else:
            raise ValueError("unexpected capture record")
    if (
        bodies != [["BODY", "0", "1", "1", "4"]]
        or len(faces) != 1
        or faces[0][1:3] != ["0", "0"]
    ):
        raise ValueError("expected one four-edge sheet face")
    if (
        set(edges) != {(0, i) for i in range(4)}
        or set(samples) != set(edges)
        or set(supports) != set(edges)
    ):
        raise ValueError("incomplete API coverage")
    # BCURVE_TYPE is 3005; 3006 is SPCURVE_TYPE, used by the earlier
    # surface-boundary fixture. Neither identity alone certifies native NURBS.
    if not any(edge[0] in (3005, 3006) for edge in edges.values()):
        raise ValueError("expected at least one spline API boundary")
    for identity, edge in edges.items():
        if (
            edge[0] not in (3001, 3005, 3006)
            or edge[1] not in (0, -1)
            or edge[2] >= edge[3]
        ):
            raise ValueError("expected directed line or spline API interval")
        if supports[identity][0] != -1 or supports[identity][3:] != [0, 0]:
            raise ValueError("expected nonperiodic open API support")
        if set(samples[identity]) != set(range(17)):
            raise ValueError("incomplete API samples")
        for i, sample in samples[identity].items():
            if abs(sample[0] - (edge[2] + (edge[3] - edge[2]) * i / 16)) > 1e-10:
                raise ValueError("API sample parameter mismatch")
        for i, endpoint in ((0, edge[4:7]), (16, edge[7:10])):
            if distance(samples[identity][i][1:4], endpoint) * 1000 > TOLERANCE_MM:
                raise ValueError("API endpoints disagree with samples")
    return edges, samples, supports


def read_curve(definition):
    if definition["kind"] == "nurbs":
        curve = Nurbs.read("curve", definition)
        axis = curve.axes[0]

        def evaluate(t):
            point, (tangent,) = curve.evaluate((t,))
            return point, tangent

        return evaluate, [axis.knots[axis.degree], axis.knots[axis.count]], (3005, 3006)
    if definition["kind"] == "line":
        origin = [definition["origin"][axis] for axis in "xyz"]
        direction = [definition["direction"][axis] for axis in "xyz"]
        if (
            not all(math.isfinite(v) for v in origin + direction)
            or math.hypot(*direction) == 0
        ):
            raise ValueError("invalid native line")

        def evaluate(t):
            return [
                p + t * d for p, d in zip(origin, direction, strict=True)
            ], direction

        return evaluate, None, (3001,)
    raise ValueError("expected NURBS or line boundary carrier")


def validate(model, capture):
    edges, samples, supports = capture
    if [len(model[k]) for k in ("bodies", "faces", "edges")] != [1, 1, 4]:
        raise ValueError("expected one decoded sheet body, one face and four edges")
    carriers = {v["id"]: v for v in model["carriers"]}
    points = {v["id"]: v["position"] for v in model["points"]}
    vertices = {v["id"]: points[v["point_id"]] for v in model["vertices"]}
    used, checks = set(), []
    for edge in model["edges"]:
        definition = carriers[edge["curve_id"]]["definition"]
        evaluate, domain, api_types = read_curve(definition)
        derived = edge.get("derived_parameter_interval")
        bounds = edge.get("parameter_range") or (derived or {}).get("parameter_range")
        if (
            bounds is None
            or len(bounds) != 2
            or not all(math.isfinite(v) for v in bounds)
            or bounds[0] == bounds[1]
        ):
            raise ValueError("missing or invalid effective interval")
        ends = [evaluate(t)[0] for t in bounds]
        endpoint_error = max(
            distance(p, vertices[edge[key]])
            for p, key in zip(ends, ("start_vertex_id", "end_vertex_id"), strict=True)
        )
        matches = []
        for identity in edges:
            if edges[identity][0] not in api_types:
                continue
            api = [[v * 1000 for v in samples[identity][i][1:4]] for i in (0, 16)]
            for reverse in (False, True):
                if (
                    max(distance(ends[i], api[1 - i if reverse else i]) for i in (0, 1))
                    <= TOLERANCE_MM
                ):
                    matches.append((identity, reverse))
        if len(matches) != 1 or matches[0][0] in used:
            raise ValueError("non-bijective API/native edge match")
        identity, reverse = matches[0]
        used.add(identity)
        errors, dots = [], []
        lo, hi = bounds[::-1] if reverse else bounds
        for i, sample in samples[identity].items():
            point, tangent = evaluate(lo + (hi - lo) * i / 16)
            reference = [v * 1000 for v in sample[1:4]]
            reference_tangent = sample[4:7]
            rate = (hi - lo) / (edges[identity][3] - edges[identity][2])
            tangent = [v * rate for v in tangent]
            denominator = math.hypot(*tangent) * math.hypot(*reference_tangent)
            if denominator <= 0:
                raise ValueError("degenerate API/native tangent")
            errors.append(distance(point, reference))
            dots.append(
                sum(a * b for a, b in zip(tangent, reference_tangent, strict=True))
                / denominator
            )
        partial = (
            domain is not None
            and max(abs(a - b) for a, b in zip(sorted(bounds), domain, strict=True))
            > 1e-9
        )
        checks.append(
            {
                "edge_id": edge["id"],
                "native_curve_kind": definition["kind"],
                "api_curve_type": int(edges[identity][0]),
                "api_edge": list(identity),
                "native_interval": bounds,
                "native_support_domain": domain,
                "api_interval": edges[identity][2:4],
                "api_support_domain": supports[identity][1:3],
                "partial_support_interval": partial,
                "method": (derived or {}).get("method", "source"),
                "endpoint_error_mm": endpoint_error,
                "max_sample_error_mm": max(errors),
                "minimum_tangent_dot": min(dots),
                "passed": endpoint_error <= TOLERANCE_MM
                and max(errors) <= TOLERANCE_MM
                and all(abs(v - 1) <= 1e-9 for v in dots),
            }
        )
    return {
        "passed": len(used) == 4 and all(c["passed"] for c in checks),
        "edges": checks,
        "samples": len(checks) * 17,
        "partial_support_intervals": sum(c["partial_support_interval"] for c in checks),
        "source_trim_verified": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--source-sha256", required=True)
    parser.add_argument("--capture-sha256", required=True)
    parser.add_argument("--require-partial", action="store_true")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.resolve() in (args.source.resolve(), args.capture.resolve()):
        parser.error("output must not overwrite evidence")
    report = {"schema_version": 1, "passed": False}
    try:
        for name in ("source", "capture"):
            path = getattr(args, name)
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            if digest != getattr(args, name + "_sha256"):
                raise ValueError(name + " SHA-256 mismatch")
            report[name] = {
                "name": path.name,
                "sha256": digest,
                "bytes": path.stat().st_size,
            }
        capture = read_capture(args.capture, args.source)
        model = sldkit.decode_geometry_file(args.source).to_dict()["geometry"]["model"]
        report.update(validate(model, capture))
        if args.require_partial and report["partial_support_intervals"] == 0:
            raise ValueError("saved native curves contain no partial support intervals")
    except (ValueError, KeyError, IndexError, OSError) as error:
        report.update(passed=False, error=str(error))
    args.output.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    print(json.dumps({k: v for k, v in report.items() if k != "edges"}))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
