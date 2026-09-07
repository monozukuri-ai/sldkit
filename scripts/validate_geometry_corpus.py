#!/usr/bin/env python3
"""Validate modern Part geometry determinism, provenance, and API parity."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import platform
import statistics
import subprocess
import time
from collections import Counter
from collections.abc import Iterable
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import sldkit
from sldkit import _core


def canonical(value: dict[str, Any]) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()


def byte_partition_errors(
    geometry: dict, bodies: dict[str, bytes] | None = None
) -> list[str]:
    """Audit interval algebra independently of Rust; optionally verify read hashes."""
    fidelity = geometry["fidelity"]
    coverage = fidelity["byte_coverage"]
    spans = fidelity.get("byte_spans", [])
    ranges = fidelity.get("byte_ranges", [])
    if coverage["partition_status"] != "complete":
        return (
            []
            if not spans and not ranges
            else ["incomplete ledger has classified ranges"]
        )
    domains = {domain["id"]: domain for domain in fidelity.get("byte_domains", [])}
    errors = []
    if not domains or len(domains) != len(fidelity["byte_domains"]):
        errors.append("missing or duplicate domains")
    for item in [*spans, *ranges]:
        if item["domain_id"] not in domains:
            errors.append("unknown range domain")
    counts = {"typed": 0, "uninterpreted": 0}
    for identity, domain in domains.items():
        reads = sorted(
            (s["offset"], s["offset"] + s["byte_len"])
            for s in spans
            if s["domain_id"] == identity and s["classification"] == "typed"
        )
        union: list[list[int]] = []
        for start, end in reads:
            if union and start <= union[-1][1]:
                union[-1][1] = max(end, union[-1][1])
            else:
                union.append([start, end])
        cursor = 0
        typed = []
        for item in (r for r in ranges if r["domain_id"] == identity):
            start, length, kind = (
                item["offset"],
                item["byte_len"],
                item["classification"],
            )
            if (
                start != cursor
                or length <= 0
                or kind not in counts
                or not item["reason"]
            ):
                errors.append(f"{identity}: invalid partition interval")
                continue
            cursor = start + length
            counts[kind] += length
            if kind == "typed":
                typed.append([start, cursor])
        if cursor != domain["byte_len"] or typed != union:
            errors.append(
                f"{identity}: partition does not equal read union plus complement"
            )
        for span in (s for s in spans if s["domain_id"] == identity):
            start, end = span["offset"], span["offset"] + span["byte_len"]
            if start < 0 or end <= start or end > domain["byte_len"] or not span["tag"]:
                errors.append(f"{identity}: invalid read interval")
            if span["classification"] not in counts:
                errors.append(f"{identity}: invalid read classification")
            if span["classification"] == "uninterpreted" and any(
                a < end and start < b for a, b in union
            ):
                errors.append(f"{identity}: conflicting read classifications")
            if (
                bodies is not None
                and hashlib.sha256(bodies[identity][start:end]).hexdigest()
                != span["sha256"]
            ):
                errors.append(f"{identity}: read hash mismatch")
    total = sum(domain["byte_len"] for domain in domains.values())
    if (
        counts["typed"] != coverage["typed_bytes"]
        or counts["uninterpreted"] != coverage["uninterpreted_bytes"]
        or sum(counts.values()) != total
        or coverage["partition_domain_bytes"] != total
        or coverage["classified_active_bytes"] != total
        or coverage["unclassified_active_bytes"] != 0
    ):
        errors.append("partition counters disagree with intervals")
    return errors


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def timed_python(path: Path, profile: str) -> tuple[dict[str, Any], float]:
    started = time.perf_counter()
    value = sldkit.decode_geometry_file(path, profile=profile).to_dict()
    return value, (time.perf_counter() - started) * 1000.0


def timed_rust(
    path: Path, executable: Path, profile: str
) -> tuple[dict[str, Any], float]:
    started = time.perf_counter()
    completed = subprocess.run(
        [str(executable), "geometry", str(path), "--limits", profile],
        check=False,
        capture_output=True,
        text=True,
    )
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(
            f"Rust geometry decoder failed with exit {completed.returncode}: {detail}"
        )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise TypeError("Rust geometry decoder returned non-object JSON")
    return value, elapsed_ms


def topology_reference_errors(model: dict[str, Any]) -> list[str]:
    bodies = keyed(model, "bodies")
    regions = keyed(model, "regions")
    shells = keyed(model, "shells")
    faces = keyed(model, "faces")
    loops = keyed(model, "loops")
    coedges = keyed(model, "coedges")
    edges = keyed(model, "edges")
    vertices = keyed(model, "vertices")
    points = keyed(model, "points")
    carriers = keyed(model, "carriers")
    constructions = keyed(model, "constructions")
    errors: list[str] = []

    for body in bodies.values():
        require_ids(errors, body["id"], body["region_ids"], regions)
    for region in regions.values():
        require_id(errors, region["id"], region["body_id"], bodies)
        require_ids(errors, region["id"], region["shell_ids"], shells)
    for shell in shells.values():
        require_id(errors, shell["id"], shell["region_id"], regions)
        require_ids(errors, shell["id"], shell["face_ids"], faces)
        require_ids(errors, shell["id"], shell.get("wire_edge_ids", []), edges)
        require_ids(errors, shell["id"], shell.get("free_vertex_ids", []), vertices)
    for face in faces.values():
        require_id(errors, face["id"], face["shell_id"], shells)
        require_id(errors, face["id"], face["surface_id"], carriers)
        require_ids(errors, face["id"], face["loop_ids"], loops)
    for loop in loops.values():
        require_id(errors, loop["id"], loop["face_id"], faces)
        require_ids(errors, loop["id"], loop.get("coedge_ids", []), coedges)
        for use in loop.get("vertex_uses", []):
            require_id(errors, loop["id"], use["vertex_id"], vertices)
            if use.get("after_coedge_id") is not None:
                require_id(errors, loop["id"], use["after_coedge_id"], coedges)
            check_pcurves(errors, loop["id"], use.get("pcurves", []), carriers)
    for coedge in coedges.values():
        require_id(errors, coedge["id"], coedge["loop_id"], loops)
        require_id(errors, coedge["id"], coedge["edge_id"], edges)
        for field in ("next_id", "previous_id", "radial_next_id"):
            require_id(errors, coedge["id"], coedge[field], coedges)
        if coedge.get("use_curve_id") is not None:
            require_id(errors, coedge["id"], coedge["use_curve_id"], carriers)
        check_pcurves(errors, coedge["id"], coedge.get("pcurves", []), carriers)
    for edge in edges.values():
        if edge.get("curve_id") is not None:
            require_id(errors, edge["id"], edge["curve_id"], carriers)
        require_id(errors, edge["id"], edge["start_vertex_id"], vertices)
        require_id(errors, edge["id"], edge["end_vertex_id"], vertices)
    for vertex in vertices.values():
        require_id(errors, vertex["id"], vertex["point_id"], points)
    for carrier in carriers.values():
        if carrier["kind"] == "procedural":
            construction_id = carrier["definition"].get("construction")
            if not isinstance(construction_id, str):
                errors.append(
                    f"{carrier['id']} has no procedural construction identity"
                )
            else:
                require_id(errors, carrier["id"], construction_id, constructions)
    for construction in constructions.values():
        produced_carrier_id = construction["produced_carrier_id"]
        require_id(errors, construction["id"], produced_carrier_id, carriers)
        produced = carriers.get(produced_carrier_id)
        if produced is not None and produced["domain"] != construction["domain"]:
            errors.append(
                f"{construction['id']} domain does not match {produced_carrier_id}"
            )
    for mesh in model.get("tessellations", []):
        if mesh.get("body_id") is not None:
            require_id(errors, mesh["id"], mesh["body_id"], bodies)
        require_ids(errors, mesh["id"], mesh.get("face_ids", []), faces)
        vertex_count = len(mesh.get("vertices", []))
        triangle_count = len(mesh.get("triangles", []))
        for triangle in mesh.get("triangles", []):
            if any(index < 0 or index >= vertex_count for index in triangle):
                errors.append(f"{mesh['id']} has an invalid triangle vertex index")
        for feature_edge in mesh.get("feature_edges", []):
            if any(index < 0 or index >= vertex_count for index in feature_edge):
                errors.append(f"{mesh['id']} has an invalid feature-edge vertex index")
        groups = [
            *mesh.get("triangle_groups", []),
            *mesh.get("texture_assignments", []),
        ]
        for group in groups:
            if any(
                index < 0 or index >= triangle_count
                for index in group.get("triangles", [])
            ):
                errors.append(f"{mesh['id']} has an invalid triangle-group index")
    return sorted(set(errors))


def keyed(model: dict[str, Any], name: str) -> dict[str, dict[str, Any]]:
    return {str(item["id"]): item for item in model.get(name, [])}


def require_id(
    errors: list[str], owner: str, target: str, arena: dict[str, Any]
) -> None:
    if target not in arena:
        errors.append(f"{owner} references missing {target}")


def require_ids(
    errors: list[str], owner: str, targets: Iterable[str], arena: dict[str, Any]
) -> None:
    for target in targets:
        require_id(errors, owner, target, arena)


def check_pcurves(
    errors: list[str],
    owner: str,
    uses: Iterable[dict[str, Any]],
    carriers: dict[str, Any],
) -> None:
    for use in uses:
        require_id(errors, owner, use["pcurve_id"], carriers)


def topology_metrics_valid(geometry: dict[str, Any]) -> bool:
    body_ids = {item["id"] for item in geometry["model"]["bodies"]}
    metrics = geometry.get("topology_metrics", [])
    if {item["body_id"] for item in metrics} != body_ids:
        return False
    return all(
        item.get("euler_characteristic") is None
        or item["euler_characteristic"]
        == item["vertices"] - item["edges"] + item["faces"]
        for item in metrics
    )


def _finite_number(value: Any) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
    )


def _points_valid(value: Any, count: int) -> bool:
    return (
        isinstance(value, list)
        and len(value) == count
        and all(
            isinstance(point, dict)
            and bool(point)
            and all(_finite_number(coordinate) for coordinate in point.values())
            for point in value
        )
    )


def _knots_valid(value: Any, expected_count: int) -> bool:
    return (
        isinstance(value, list)
        and len(value) == expected_count
        and all(_finite_number(knot) for knot in value)
        and all(left <= right for left, right in zip(value, value[1:], strict=False))
    )


def _weights_valid(value: Any, count: int, *, positive: bool) -> bool:
    if value is None:
        return True
    return (
        isinstance(value, list)
        and len(value) == count
        and all(
            _finite_number(weight) and (weight > 0.0 if positive else weight != 0.0)
            for weight in value
        )
    )


def carrier_parameter_errors(model: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for carrier in model.get("carriers", []):
        kind = carrier.get("kind")
        if kind not in {"nurbs", "polar_nurbs"}:
            continue
        identity = str(carrier.get("id"))
        domain = carrier.get("domain")
        definition = carrier.get("definition")
        if not isinstance(definition, dict) or definition.get("kind") != kind:
            errors.append(f"{identity} has an invalid NURBS definition tag")
            continue

        if domain == "surface" and kind == "nurbs":
            u_count = definition.get("u_count")
            v_count = definition.get("v_count")
            u_degree = definition.get("u_degree")
            v_degree = definition.get("v_degree")
            shape_valid = all(
                isinstance(value, int) and not isinstance(value, bool)
                for value in (u_count, v_count, u_degree, v_degree)
            )
            if not shape_valid:
                errors.append(f"{identity} has non-integer NURBS surface shape")
                continue
            pole_count = u_count * v_count
            valid = (
                u_degree > 0
                and v_degree > 0
                and u_count > u_degree
                and v_count > v_degree
                and _points_valid(definition.get("control_points"), pole_count)
                and _knots_valid(definition.get("u_knots"), u_count + u_degree + 1)
                and _knots_valid(definition.get("v_knots"), v_count + v_degree + 1)
                and _weights_valid(
                    definition.get("weights"), pole_count, positive=False
                )
            )
            if not valid:
                errors.append(f"{identity} has inconsistent NURBS surface parameters")
            continue

        degree = definition.get("degree")
        if not isinstance(degree, int) or isinstance(degree, bool) or degree <= 0:
            errors.append(f"{identity} has an invalid NURBS curve degree")
            continue
        points_name = (
            "radial_control_points" if kind == "polar_nurbs" else "control_points"
        )
        points = definition.get(points_name)
        point_count = len(points) if isinstance(points, list) else 0
        valid = (
            point_count > degree
            and _points_valid(points, point_count)
            and _knots_valid(definition.get("knots"), point_count + degree + 1)
            and _weights_valid(
                definition.get("weights"),
                point_count,
                positive=domain == "pcurve",
            )
        )
        if kind == "polar_nurbs":
            axial = definition.get("axial_control_points")
            valid = (
                valid
                and isinstance(axial, list)
                and len(axial) == point_count
                and all(_finite_number(value) for value in axial)
            )
        if not valid:
            errors.append(f"{identity} has inconsistent {kind} parameters")
    return sorted(errors)


def stream_extraction_valid(path: Path, geometry: dict[str, Any], profile: str) -> bool:
    for stream in geometry.get("source_streams", []):
        extracted = sldkit.extract_file(path, stream["entry_id"], profile=profile)
        if extracted.result.status is not sldkit.ExtractionStatus.EXTRACTED:
            return False
        if extracted.data is None:
            return False
        if len(extracted.data) != stream.get("decoded_size"):
            return False
        if hashlib.sha256(extracted.data).hexdigest() != stream.get("decoded_sha256"):
            return False
    return True


def referenced_raw_records_preserved(
    path: Path, geometry: dict[str, Any], profile: str
) -> bool:
    raw_records = {item["id"]: item for item in geometry.get("raw_records", [])}
    streams = {item["stream_path"]: item for item in geometry.get("source_streams", [])}
    decoded_streams: dict[str, bytes] = {}
    carriers = geometry["model"].get("carriers", [])
    constructions = geometry["model"].get("constructions", [])
    record_ids = [
        item.get("raw_record_id") for item in carriers if item["kind"] == "unknown"
    ]
    record_ids.extend(
        item["raw_record_id"]
        for item in constructions
        if item.get("raw_record_id") is not None
    )
    if any(
        item.get("definition", {}).get("kind") == "unknown"
        and item.get("raw_record_id") is None
        for item in constructions
    ):
        return False
    for record_id in record_ids:
        record = raw_records.get(record_id)
        if record is None:
            return False
        stream = streams.get(record["stream"])
        if stream is None:
            return False
        if record["stream"] not in decoded_streams:
            extracted = sldkit.extract_file(path, stream["entry_id"], profile=profile)
            if extracted.data is None:
                return False
            decoded_streams[record["stream"]] = extracted.data
        data = decoded_streams[record["stream"]]
        start = int(record["offset"])
        end = start + int(record["byte_len"])
        if end > len(data):
            return False
        if hashlib.sha256(data[start:end]).hexdigest() != record["sha256"]:
            return False
    return True


def case(path: Path, executable: Path, profile: str) -> tuple[dict[str, Any], bool]:
    first, first_ms = timed_python(path, profile)
    second, second_ms = timed_python(path, profile)
    native_started = time.perf_counter()
    native = json.loads(_core.decode_geometry_file_json(str(path), profile))
    native_ms = (time.perf_counter() - native_started) * 1000.0
    rust, rust_ms = timed_rust(path, executable, profile)
    geometry = first.get("geometry")
    if not isinstance(geometry, dict):
        raise RuntimeError(f"geometry decoder returned no document for {path}")

    deterministic = canonical(first) == canonical(second)
    native_parity = canonical(first) == canonical(native)
    rust_parity = canonical(first) == canonical(rust)
    reference_errors = topology_reference_errors(geometry["model"])
    metrics_valid = topology_metrics_valid(geometry)
    parameter_errors = carrier_parameter_errors(geometry["model"])
    extraction_valid = stream_extraction_valid(path, geometry, profile)
    raw_records_preserved = referenced_raw_records_preserved(path, geometry, profile)
    active_partition_count = sum(
        stream["role"] == "parasolid_partition" and stream["selection"] == "active"
        for stream in geometry.get("source_streams", [])
    )
    byte_coverage = geometry["fidelity"]["byte_coverage"]
    byte_domains = geometry["fidelity"].get("byte_domains", [])
    byte_domain_ids = [domain["id"] for domain in byte_domains]
    byte_domains_sane = (
        len(byte_domain_ids) == len(set(byte_domain_ids))
        and all(
            domain["role"] in {"parasolid_partition", "parasolid_deltas"}
            and domain["offset_basis"] == "parasolid_body"
            and int(domain["body_offset"]) + int(domain["byte_len"])
            == int(domain["stream_byte_len"])
            and len(domain["sha256"]) == 64
            and len(domain["stream_sha256"]) == 64
            for domain in byte_domains
        )
        and sum(int(domain["byte_len"]) for domain in byte_domains)
        == byte_coverage["partition_domain_bytes"]
    )
    partition_errors = byte_partition_errors(geometry)
    byte_coverage_sane = (
        not partition_errors
        and byte_coverage["source_bytes"] == path.stat().st_size
        and byte_coverage["active_stream_bytes"]
        <= byte_coverage["candidate_stream_bytes"]
        and byte_coverage["classified_active_bytes"]
        + byte_coverage["unclassified_active_bytes"]
        == byte_coverage["partition_domain_bytes"]
        and byte_coverage["unique_location_count"]
        <= byte_coverage["located_entity_count"]
        and (
            byte_coverage["partition_status"] != "complete"
            or (
                byte_coverage["unclassified_active_bytes"] == 0
                and byte_coverage["typed_bytes"] is not None
                and byte_coverage["uninterpreted_bytes"] is not None
                and byte_coverage["typed_bytes"] + byte_coverage["uninterpreted_bytes"]
                == byte_coverage["partition_domain_bytes"]
            )
        )
    )
    passed = (
        first["status"] in {"decoded", "partial"}
        and geometry["fidelity"]["geometry_transferred"]
        and deterministic
        and native_parity
        and rust_parity
        and not reference_errors
        and metrics_valid
        and not parameter_errors
        and extraction_valid
        and raw_records_preserved
        and active_partition_count >= 1
        and bool(byte_domains)
        and byte_domains_sane
        and byte_coverage_sane
    )

    model = geometry["model"]
    carrier_kinds = Counter(item["kind"] for item in model["carriers"])
    loss_categories = Counter(
        item["category"] for item in geometry["fidelity"]["losses"]
    )
    record = {
        "source_name": path.name,
        "source_bytes": path.stat().st_size,
        "source_sha256": sha256_file(path),
        "status": first["status"],
        "decoder": geometry["fidelity"]["decoder"],
        "decoder_version": geometry["fidelity"]["decoder_version"],
        "entity_counts": {
            name: len(model[name])
            for name in (
                "bodies",
                "regions",
                "shells",
                "faces",
                "loops",
                "coedges",
                "edges",
                "vertices",
                "points",
                "carriers",
                "constructions",
                "tessellations",
            )
        },
        "carrier_kinds": dict(sorted(carrier_kinds.items())),
        "configuration_count": len(geometry.get("configurations", [])),
        "topology_metrics": geometry.get("topology_metrics", []),
        "loss_categories": dict(sorted(loss_categories.items())),
        "loss_codes": [item["code"] for item in geometry["fidelity"]["losses"]],
        "diagnostic_codes": [item["code"] for item in first["diagnostics"]],
        "byte_domains": byte_domains,
        "byte_coverage": byte_coverage,
        "byte_domains_sane": byte_domains_sane,
        "byte_partition_errors": partition_errors,
        "active_partition_count": active_partition_count,
        "deterministic": deterministic,
        "python_native_json_parity": native_parity,
        "rust_cli_json_parity": rust_parity,
        "topology_reference_errors": reference_errors,
        "topology_metrics_valid": metrics_valid,
        "carrier_parameter_errors": parameter_errors,
        "stream_extraction_valid": extraction_valid,
        "referenced_raw_records_preserved": raw_records_preserved,
        "byte_coverage_sane": byte_coverage_sane,
        "elapsed_ms": {
            "python": [round(first_ms, 3), round(second_ms, 3)],
            "native": round(native_ms, 3),
            "rust_cli": round(rust_ms, 3),
        },
    }
    return record, passed


def distribution(values: list[float]) -> dict[str, float]:
    ordered = sorted(values)
    p95_index = max(0, (95 * len(ordered) + 99) // 100 - 1)
    return {
        "min": round(ordered[0], 3),
        "median": round(statistics.median(ordered), 3),
        "p95": round(ordered[p95_index], 3),
        "max": round(ordered[-1], 3),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rust-cli", type=Path, required=True)
    parser.add_argument("--profile", choices=("desktop", "service"), default="service")
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--build-profile",
        choices=("unknown", "debug", "release"),
        default="unknown",
        help="Build profile of both the native extension and Rust CLI being measured",
    )
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()

    records = []
    passed = True
    for path in args.paths:
        record, case_passed = case(path, args.rust_cli, args.profile)
        records.append(record)
        passed = passed and case_passed
    python_times = [
        elapsed for record in records for elapsed in record["elapsed_ms"]["python"]
    ]
    rust_times = [record["elapsed_ms"]["rust_cli"] for record in records]
    report = {
        "schema_version": 1,
        "captured_at": datetime.now(timezone.utc).isoformat(),
        "passed": passed,
        "profile": args.profile,
        "case_count": len(records),
        "environment": {
            "system": platform.system(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "sldkit": sldkit.__version__,
            "rust_cli": args.rust_cli.name,
            "timing_scope": "validation_only",
            "build_profile": args.build_profile,
        },
        "performance_ms": {
            "python": distribution(python_times),
            "rust_cli": distribution(rust_times),
        },
        "cases": records,
    }
    output = json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.output is None:
        print(output, end="")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(output, encoding="utf-8")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
