"""Compare decoded B-Rep edges with the saved display-mesh face outlines.

The display tables are SolidWorks' own face tessellations, so their outlines
are an independent witness of the face boundaries in every saved Part. This
tool measures, without a geometry kernel:

- ``edge_on_display``: the fraction of sampled B-Rep edge points that lie on a
  display outline within the tolerance (wrong curves or placement lower it);
- ``outline_covered``: the fraction of display outline points that lie on a
  sampled B-Rep edge within the tolerance (missing faces or edges lower it).

Derived periodic seams are excluded because display outlines weld them away.
Circle edges that the kernel-free evaluator leaves unsampled for lack of a
verified interval are resolved here only for measurement: both arcs between the
endpoints (or the full ring) are sampled and the arc lying on the display
outlines is kept. Their count is reported as ``display_resolved_arcs``; the
choice is a witness for this comparison, not a decoded trim.
The tolerance is ``--relative-tolerance`` times the display bounding-box
diagonal, but never below three chord tolerances. The verdict is ``match`` when
both fractions reach ``--match``; otherwise the lower fraction names the gap.

Usage::

    uv run python scripts/validate_display_oracle.py --profile service part.SLDPRT ...
"""

from __future__ import annotations

import argparse
import array
import json
import math
import random
import sys
from collections import defaultdict
from collections.abc import Iterable, Sequence
from pathlib import Path

import sldkit

Point = tuple[float, float, float]
Segment = tuple[Point, Point]


def _distance_to_segment(point: Point, segment: Segment) -> float:
    (ax, ay, az), (bx, by, bz) = segment
    px, py, pz = point
    dx, dy, dz = bx - ax, by - ay, bz - az
    length2 = dx * dx + dy * dy + dz * dz
    t = 0.0
    if length2 > 0.0:
        t = max(
            0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy + (pz - az) * dz) / length2)
        )
    cx, cy, cz = ax + t * dx - px, ay + t * dy - py, az + t * dz - pz
    return math.sqrt(cx * cx + cy * cy + cz * cz)


class SegmentGrid:
    """Uniform grid of segment pieces no longer than one cell."""

    def __init__(self, segments: Iterable[Segment], cell: float) -> None:
        self.cell = cell
        self.cells: dict[tuple[int, int, int], list[Segment]] = defaultdict(list)
        for start, end in segments:
            length = math.dist(start, end)
            pieces = max(1, math.ceil(length / cell))
            previous = start
            for index in range(1, pieces + 1):
                t = index / pieces
                current = (
                    start[0] + (end[0] - start[0]) * t,
                    start[1] + (end[1] - start[1]) * t,
                    start[2] + (end[2] - start[2]) * t,
                )
                key = self._key(
                    (
                        (previous[0] + current[0]) / 2,
                        (previous[1] + current[1]) / 2,
                        (previous[2] + current[2]) / 2,
                    )
                )
                self.cells[key].append((previous, current))
                previous = current

    def _key(self, point: Point) -> tuple[int, int, int]:
        return (
            math.floor(point[0] / self.cell),
            math.floor(point[1] / self.cell),
            math.floor(point[2] / self.cell),
        )

    def nearest(self, point: Point) -> float:
        """Distance to the nearest piece within two cells, else infinity."""
        kx, ky, kz = self._key(point)
        best = math.inf
        for x in range(kx - 2, kx + 3):
            for y in range(ky - 2, ky + 3):
                for z in range(kz - 2, kz + 3):
                    for segment in self.cells.get((x, y, z), ()):
                        best = min(best, _distance_to_segment(point, segment))
        return best


def _sample(items: Sequence[Point], limit: int, seed: int) -> Sequence[Point]:
    if len(items) <= limit:
        return items
    return random.Random(seed).sample(list(items), limit)


def _fraction_within(distances: Sequence[float], tolerance: float) -> float | None:
    if not distances:
        return None
    return sum(1 for value in distances if value <= tolerance) / len(distances)


def _quantile(values: Sequence[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(fraction * len(ordered)))]


def display_outline(path: Path, profile: str) -> tuple[list[Segment], float]:
    result = sldkit.decode_display_mesh_file(path, profile=profile)
    if result.mesh is None or result.mesh.bounds is None:
        return [], 0.0
    positions = array.array("f", result.positions)
    edges = array.array("I", result.edges)
    points = [
        (positions[i * 3], positions[i * 3 + 1], positions[i * 3 + 2])
        for i in range(len(positions) // 3)
    ]
    segments = [
        (points[edges[i]], points[edges[i + 1]]) for i in range(0, len(edges), 2)
    ]
    diagonal = math.dist(result.mesh.bounds.min, result.mesh.bounds.max)
    return segments, diagonal


def _unit(vector: Sequence[float]) -> Point:
    length = math.sqrt(sum(value * value for value in vector))
    return tuple(value / length for value in vector) if length > 0 else (0.0, 0.0, 0.0)


def _cross(left: Sequence[float], right: Sequence[float]) -> Point:
    return (
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    )


def _arc_candidates(
    definition: dict, start: Point, end: Point, closed: bool, chord: float
) -> list[list[Point]]:
    """Both arcs from ``start`` to ``end`` on a circle carrier, or its full ring."""
    try:
        center = tuple(float(definition["center"][axis]) for axis in "xyz")
        axis = _unit([float(definition["axis"][key]) for key in "xyz"])
        reference = _unit([float(definition["ref_direction"][key]) for key in "xyz"])
        radius = float(definition["radius"])
    except (KeyError, TypeError, ValueError):
        return []
    across = _cross(axis, reference)

    def angle(point: Point) -> float:
        offset = [point[index] - center[index] for index in range(3)]
        return math.atan2(
            sum(o * a for o, a in zip(offset, across, strict=True)),
            sum(o * r for o, r in zip(offset, reference, strict=True)),
        )

    def arc(begin: float, sweep: float) -> list[Point]:
        steps = max(8, math.ceil(abs(sweep) * radius / max(chord, 1e-6) / 4))
        steps = min(steps, 720)
        points = []
        for index in range(steps + 1):
            theta = begin + sweep * index / steps
            cos, sin = math.cos(theta), math.sin(theta)
            points.append(
                tuple(
                    center[axis_index]
                    + radius * (cos * reference[axis_index] + sin * across[axis_index])
                    for axis_index in range(3)
                )
            )
        return points

    first = angle(start)
    if closed:
        return [arc(first, 2 * math.pi)]
    counter = (angle(end) - first) % (2 * math.pi)
    if counter == 0.0:
        return [arc(first, 2 * math.pi)]
    return [arc(first, counter), arc(first, counter - 2 * math.pi)]


def brep_polylines(
    path: Path, profile: str, chord: float, display_grid: SegmentGrid | None
) -> tuple[list[Segment], dict]:
    decoded = sldkit.decode_geometry_file(path, profile=profile)
    document = decoded.geometry
    if document is None:
        return [], {"edges": 0, "sampled": 0, "seams": 0, "display_resolved_arcs": 0}
    evaluation = sldkit.evaluate_geometry(document, chord_tolerance_mm=chord).evaluation
    if evaluation is None:
        return [], {"edges": 0, "sampled": 0, "seams": 0, "display_resolved_arcs": 0}
    model = document.model
    carriers = {carrier.id: carrier for carrier in model.carriers}
    points = {point.id: point.position for point in model.points}
    vertices = {vertex.id: points.get(vertex.point_id) for vertex in model.vertices}
    edges = {edge.id: edge for edge in model.edges}
    segments: list[Segment] = []
    sampled = seams = resolved = 0
    for evaluated in evaluation.edges:
        if evaluated.is_derived_seam:
            seams += 1
            continue
        polyline = list(evaluated.polyline)
        if len(polyline) < 2 and display_grid is not None:
            edge = edges.get(evaluated.edge_id)
            carrier = carriers.get(edge.curve_id) if edge and edge.curve_id else None
            start = vertices.get(edge.start_vertex_id) if edge else None
            end = vertices.get(edge.end_vertex_id) if edge else None
            if carrier is not None and carrier.kind == "circle" and start and end:
                closed = edge.start_vertex_id == edge.end_vertex_id
                candidates = _arc_candidates(
                    carrier.definition, start, end, closed, chord
                )
                scored = [
                    (
                        _fraction_within(
                            [display_grid.nearest(point) for point in candidate],
                            display_grid.cell,
                        )
                        or 0.0,
                        candidate,
                    )
                    for candidate in candidates
                ]
                if scored:
                    polyline = max(scored, key=lambda item: item[0])[1]
                    resolved += 1
        if len(polyline) < 2:
            continue
        sampled += 1
        segments.extend(zip(polyline, polyline[1:], strict=False))
    return segments, {
        "edges": len(evaluation.edges),
        "sampled": sampled,
        "seams": seams,
        "display_resolved_arcs": resolved,
    }


def compare(
    path: Path, profile: str, chord: float, relative: float, limit: int
) -> dict:
    outline, diagonal = display_outline(path, profile)
    record: dict = {"file": path.name, "display_segments": len(outline)}
    if not outline:
        record["verdict"] = "no_display_mesh"
        return record
    tolerance = max(relative * diagonal, 3 * chord)
    record["tolerance_mm"] = round(tolerance, 6)
    display_grid = SegmentGrid(outline, tolerance)
    brep, counts = brep_polylines(path, profile, chord, display_grid)
    record.update(
        brep_segments=len(brep), **{f"brep_{k}": v for k, v in counts.items()}
    )
    if not brep:
        record["verdict"] = "no_brep_edges"
        return record
    brep_grid = SegmentGrid(brep, tolerance)
    brep_points = _sample([point for segment in brep for point in segment], limit, 1)
    outline_points = _sample(
        [
            tuple((a + b) / 2 for a, b in zip(start, end, strict=True))
            for start, end in outline
        ],
        limit,
        2,
    )
    on_display = [display_grid.nearest(point) for point in brep_points]
    covered = [brep_grid.nearest(point) for point in outline_points]
    record["edge_on_display"] = _fraction_within(on_display, tolerance)
    record["outline_covered"] = _fraction_within(covered, tolerance)
    finite = [value / diagonal for value in on_display if math.isfinite(value)]
    record["edge_p95_relative"] = _quantile(finite, 0.95)
    return record


def verdict(record: dict, threshold: float) -> str:
    if "verdict" in record:
        return record["verdict"]
    on_display = record["edge_on_display"] or 0.0
    covered = record["outline_covered"] or 0.0
    if on_display >= threshold and covered >= threshold:
        return "match"
    return "brep_off_display" if on_display < covered else "brep_incomplete"


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument("--profile", choices=("desktop", "service"), default="service")
    parser.add_argument(
        "--chord", type=float, default=0.05, help="edge chord tolerance (mm)"
    )
    parser.add_argument("--relative-tolerance", type=float, default=0.002)
    parser.add_argument("--match", type=float, default=0.98)
    parser.add_argument("--max-points", type=int, default=20000)
    args = parser.parse_args(argv)
    for path in args.paths:
        record = compare(
            path, args.profile, args.chord, args.relative_tolerance, args.max_points
        )
        record["verdict"] = verdict(record, args.match)
        print(json.dumps(record, ensure_ascii=False, sort_keys=True), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
