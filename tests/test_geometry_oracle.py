from __future__ import annotations

import json
import runpy
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).parents[1]


def _namespace() -> dict:
    return runpy.run_path(str(ROOT / "scripts/validate_geometry_oracle.py"))


def test_geometry_oracle_schema_is_valid():
    schema = json.loads(
        (ROOT / "docs/schemas/geometry-oracle.schema.json").read_text(encoding="utf-8")
    )
    Draft202012Validator.check_schema(schema)


def test_tessellation_metrics_for_unit_cube():
    namespace = _namespace()
    vertices = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ]
    triangles = [
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [3, 7, 6],
        [3, 6, 2],
        [0, 4, 7],
        [0, 7, 3],
        [1, 2, 6],
        [1, 6, 5],
    ]

    metrics = namespace["tessellation_metrics"](
        {"tessellations": [{"vertices": vertices, "triangles": triangles}]}
    )

    assert metrics["volume_mm3"] == pytest.approx(1.0)
    assert metrics["surface_area_mm2"] == pytest.approx(6.0)
    assert metrics["center_of_mass_mm"] == pytest.approx([0.5, 0.5, 0.5])
    assert metrics["bounds_mm"] == [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]


def test_scalar_and_vector_tolerances_are_explicit():
    namespace = _namespace()

    scalar = namespace["scalar_comparison"](
        100.0,
        100.15,
        {"absolute": 0.01, "relative": 0.002},
    )
    vector = namespace["vector_comparison"](
        [0.0, 1.0, 2.0],
        [0.001, 1.002, 1.999],
        0.002,
    )

    assert scalar["passed"]
    assert scalar["allowed_difference"] == 0.2
    assert vector["passed"]
    assert vector["maximum_absolute_difference"] == 0.0020000000000000018
