#!/usr/bin/env python3
"""Compare native Part geometry with independently captured neutral B-Rep facts."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
from collections import Counter
from collections.abc import Mapping, Sequence
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import sldkit
from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).parents[1]
SCHEMA = ROOT / "docs/schemas/geometry-oracle.schema.json"


class OracleValidationError(ValueError):
    """The oracle or its source checkout is not valid for comparison."""


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8-sig"))


def sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_schema(oracle: Any) -> None:
    schema = load_json(SCHEMA)
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(oracle), key=lambda error: error.json_path)
    if errors:
        error = errors[0]
        raise OracleValidationError(f"oracle {error.json_path}: {error.message}")


def checkout_path(checkout: Path, raw_path: str) -> Path:
    root = checkout.resolve(strict=True)
    path = (root / raw_path).resolve(strict=True)
    try:
        path.relative_to(root)
    except ValueError as error:
        raise OracleValidationError(
            f"oracle artifact escapes checkout: {raw_path}"
        ) from error
    if not path.is_file():
        raise OracleValidationError(f"oracle artifact is not a file: {raw_path}")
    return path


def git_revision(checkout: Path) -> str:
    completed = subprocess.run(
        ["git", "-C", str(checkout), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise OracleValidationError(f"cannot read checkout revision: {detail}")
    return completed.stdout.strip()


def _sub(a: Sequence[float], b: Sequence[float]) -> tuple[float, float, float]:
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def _cross(a: Sequence[float], b: Sequence[float]) -> tuple[float, float, float]:
    return (
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )


def _dot(a: Sequence[float], b: Sequence[float]) -> float:
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def tessellation_metrics(model: Mapping[str, Any]) -> dict[str, Any]:
    area = 0.0
    signed_volume = 0.0
    first_moments = [0.0, 0.0, 0.0]
    minimum = [math.inf, math.inf, math.inf]
    maximum = [-math.inf, -math.inf, -math.inf]
    vertex_count = 0
    triangle_count = 0

    for mesh in model.get("tessellations", []):
        vertices = mesh.get("vertices", [])
        triangles = mesh.get("triangles", [])
        vertex_count += len(vertices)
        triangle_count += len(triangles)
        for vertex in vertices:
            if len(vertex) != 3 or not all(math.isfinite(float(v)) for v in vertex):
                raise OracleValidationError("tessellation contains an invalid vertex")
            for axis in range(3):
                coordinate = float(vertex[axis])
                minimum[axis] = min(minimum[axis], coordinate)
                maximum[axis] = max(maximum[axis], coordinate)
        for triangle in triangles:
            if len(triangle) != 3 or any(
                not isinstance(index, int) or index < 0 or index >= len(vertices)
                for index in triangle
            ):
                raise OracleValidationError(
                    "tessellation contains an invalid triangle index"
                )
            a, b, c = (vertices[index] for index in triangle)
            normal = _cross(_sub(b, a), _sub(c, a))
            area += math.sqrt(_dot(normal, normal)) / 2.0
            tetrahedron_volume = _dot(a, _cross(b, c)) / 6.0
            signed_volume += tetrahedron_volume
            for axis in range(3):
                first_moments[axis] += (
                    tetrahedron_volume
                    * (float(a[axis]) + float(b[axis]) + float(c[axis]))
                    / 4.0
                )

    if vertex_count == 0 or triangle_count == 0:
        raise OracleValidationError("geometry has no comparable tessellation")
    center = None
    if abs(signed_volume) > 1e-15:
        center = [moment / signed_volume for moment in first_moments]
    return {
        "volume_mm3": abs(signed_volume),
        "surface_area_mm2": area,
        "center_of_mass_mm": center,
        "bounds_mm": [*minimum, *maximum],
        "tessellation_vertices": vertex_count,
        "tessellation_triangles": triangle_count,
    }


def topology_metrics(geometry: Mapping[str, Any]) -> dict[str, Any]:
    model = geometry["model"]
    body_kinds = Counter(str(body["kind"]) for body in model.get("bodies", []))
    blocking = any(
        finding.get("severity") in {"error", "blocking"}
        for finding in geometry["fidelity"].get("validation_findings", [])
    )
    return {
        "bodies_by_kind": dict(sorted(body_kinds.items())),
        "shells": len(model.get("shells", [])),
        "faces": len(model.get("faces", [])),
        "edges": len(model.get("edges", [])),
        "vertices": len(model.get("vertices", [])),
        "valid": not blocking,
    }


def scalar_comparison(
    expected: float,
    actual: float,
    tolerance: Mapping[str, Any],
) -> dict[str, Any]:
    difference = abs(actual - expected)
    allowed = max(
        float(tolerance["absolute"]),
        float(tolerance["relative"]) * abs(expected),
    )
    return {
        "expected": expected,
        "actual": actual,
        "absolute_difference": difference,
        "allowed_difference": allowed,
        "passed": difference <= allowed
        or math.isclose(difference, allowed, rel_tol=1e-12, abs_tol=1e-15),
    }


def vector_comparison(
    expected: Sequence[float] | None,
    actual: Sequence[float] | None,
    allowed: float,
) -> dict[str, Any]:
    if expected is None or actual is None:
        return {
            "expected": expected,
            "actual": actual,
            "maximum_absolute_difference": None,
            "allowed_difference": allowed,
            "passed": expected is None and actual is None,
        }
    difference = max(
        abs(float(a) - float(e)) for e, a in zip(expected, actual, strict=True)
    )
    return {
        "expected": list(expected),
        "actual": list(actual),
        "maximum_absolute_difference": difference,
        "allowed_difference": allowed,
        "passed": difference <= allowed
        or math.isclose(difference, allowed, rel_tol=1e-12, abs_tol=1e-15),
    }


def compare_pair(
    pair: Mapping[str, Any],
    checkout: Path,
    profile: str,
) -> tuple[dict[str, Any], bool]:
    native = checkout_path(checkout, str(pair["native"]["path"]))
    neutral = checkout_path(checkout, str(pair["neutral"]["path"]))
    native_hash = sha256_path(native)
    neutral_hash = sha256_path(neutral)
    integrity = {
        "native_sha256": {
            "expected": pair["native"]["sha256"],
            "actual": native_hash,
            "passed": native_hash == pair["native"]["sha256"],
        },
        "neutral_sha256": {
            "expected": pair["neutral"]["sha256"],
            "actual": neutral_hash,
            "passed": neutral_hash == pair["neutral"]["sha256"],
        },
    }
    if not all(item["passed"] for item in integrity.values()):
        return {"pair_id": pair["pair_id"], "integrity": integrity}, False

    result = sldkit.decode_geometry_file(native, profile=profile)
    if result.geometry is None:
        return {
            "pair_id": pair["pair_id"],
            "integrity": integrity,
            "status": result.status.value,
            "error": "native decoder returned no geometry document",
        }, False
    geometry = result.geometry.to_dict()
    if geometry["length_unit"] != "millimeter":
        raise OracleValidationError(
            f"unsupported geometry length unit: {geometry['length_unit']}"
        )

    actual_topology = topology_metrics(geometry)
    expected_topology = pair["reference"]["topology"]
    topology_match = actual_topology == expected_topology
    topology_required = pair["comparison"]["topology"] == "required"

    actual_geometry = tessellation_metrics(geometry["model"])
    expected_geometry = pair["reference"]["geometry"]
    tolerances = pair["comparison"]["tolerances"]
    metric_results = {
        "volume_mm3": scalar_comparison(
            float(expected_geometry["volume_mm3"]),
            float(actual_geometry["volume_mm3"]),
            tolerances["volume_mm3"],
        ),
        "surface_area_mm2": scalar_comparison(
            float(expected_geometry["surface_area_mm2"]),
            float(actual_geometry["surface_area_mm2"]),
            tolerances["surface_area_mm2"],
        ),
        "center_of_mass_mm": vector_comparison(
            expected_geometry["center_of_mass_mm"],
            actual_geometry["center_of_mass_mm"],
            float(tolerances["center_of_mass_absolute_mm"]),
        ),
        "bounds_mm": vector_comparison(
            expected_geometry["bounds_mm"],
            actual_geometry["bounds_mm"],
            float(tolerances["bounds_absolute_mm"]),
        ),
    }
    geometry_match = all(metric["passed"] for metric in metric_results.values())
    passed = geometry_match and (topology_match or not topology_required)
    return {
        "pair_id": pair["pair_id"],
        "passed": passed,
        "status": result.status.value,
        "integrity": integrity,
        "topology": {
            "mode": pair["comparison"]["topology"],
            "expected": expected_topology,
            "actual": actual_topology,
            "matched": topology_match,
        },
        "geometry": {
            "mode": pair["comparison"]["geometry"],
            "matched": geometry_match,
            "metrics": metric_results,
            "tessellation_vertices": actual_geometry["tessellation_vertices"],
            "tessellation_triangles": actual_geometry["tessellation_triangles"],
        },
        "diagnostic_codes": [diagnostic.code for diagnostic in result.diagnostics],
    }, passed


def validate(
    oracle: Mapping[str, Any],
    checkout: Path,
    profile: str,
) -> tuple[dict[str, Any], bool]:
    validate_schema(oracle)
    revision = git_revision(checkout)
    expected_revision = str(oracle["source"]["revision"])
    revision_matches = revision == expected_revision
    cases: list[dict[str, Any]] = []
    passed = revision_matches
    for pair in oracle["pairs"]:
        record, case_passed = compare_pair(pair, checkout, profile)
        cases.append(record)
        passed = passed and case_passed
    return {
        "schema_version": 1,
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "oracle_id": oracle["oracle_id"],
        "source_id": oracle["source"]["source_id"],
        "profile": profile,
        "expected_revision": expected_revision,
        "actual_revision": revision,
        "revision_matches": revision_matches,
        "case_count": len(cases),
        "passed": passed,
        "cases": cases,
    }, passed


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--oracle", required=True, type=Path)
    parser.add_argument("--checkout", required=True, type=Path)
    parser.add_argument("--profile", choices=("desktop", "service"), default="service")
    args = parser.parse_args()
    try:
        oracle = load_json(args.oracle)
        report, passed = validate(oracle, args.checkout, args.profile)
    except (
        json.JSONDecodeError,
        OSError,
        OracleValidationError,
        sldkit.SldkitError,
    ) as error:
        report = {"schema_version": 1, "passed": False, "error": str(error)}
        passed = False
    print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
