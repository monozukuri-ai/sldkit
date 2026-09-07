from __future__ import annotations

import hashlib
import json
import math
import runpy
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).parents[1]


@pytest.fixture
def tools(monkeypatch):
    monkeypatch.syspath_prepend(str(ROOT / "scripts"))
    return runpy.run_path(str(ROOT / "scripts/validate_nurbs_geometry.py"))


def quarter_circle():
    return {
        "kind": "nurbs",
        "degree": 2,
        "periodic": False,
        "control_points": [
            {"x": 1, "y": 0, "z": 0},
            {"x": 1, "y": 1, "z": 0},
            {"x": 0, "y": 1, "z": 0},
        ],
        "knots": [0, 0, 0, 1, 1, 1],
        "weights": [1, math.sqrt(0.5), 1],
    }


def quarter_cylinder():
    curve = quarter_circle()
    return {
        "kind": "nurbs",
        "u_degree": 2,
        "v_degree": 1,
        "u_count": 3,
        "v_count": 2,
        "u_periodic": False,
        "v_periodic": False,
        "u_knots": curve["knots"],
        "v_knots": [0, 0, 1, 1],
        "control_points": [
            p | {"z": z} for p in curve["control_points"] for z in (0, 3)
        ],
        "weights": [w for w in curve["weights"] for _ in range(2)],
    }


def tolerances():
    return {
        "position_mm": 1e-5,
        "parameter": 1e-12,
        "weight": 1e-12,
        "derivative_absolute_mm": 1e-7,
        "derivative_relative": 1e-9,
    }


def test_rational_curve_matches_circle_and_analytic_midpoint_derivative(tools):
    curve = tools["Nurbs"].read("curve", quarter_circle())
    for (t,) in curve.parameters(16):
        p, (d,) = curve.evaluate((t,))
        assert math.hypot(p[0], p[1]) == pytest.approx(1, abs=1e-14)
        assert sum(a * b for a, b in zip(p, d, strict=True)) == pytest.approx(
            0, abs=1e-14
        )
    p, (d,) = curve.evaluate((0.5,))
    assert p == pytest.approx((math.sqrt(0.5), math.sqrt(0.5), 0))
    speed = 2 / (1 + math.sqrt(0.5))
    assert d == pytest.approx((-speed, speed, 0))
    assert curve.evaluate((0,))[0] == (1, 0, 0)
    assert curve.evaluate((1,))[0] == (0, 1, 0)
    scaled = quarter_circle()
    scaled["weights"] = [7 * w for w in scaled["weights"]]
    assert tools["Nurbs"].read("curve", scaled).evaluate((0.5,))[0] == pytest.approx(p)


def test_rational_surface_tensor_order_and_both_derivatives(tools):
    surface = tools["Nurbs"].read("surface", quarter_cylinder())
    p, (du, dv) = surface.evaluate((0.5, 0.3))
    assert p == pytest.approx((math.sqrt(0.5), math.sqrt(0.5), 0.9))
    speed = 2 / (1 + math.sqrt(0.5))
    assert du == pytest.approx((-speed, speed, 0), abs=1e-14)
    assert dv == pytest.approx((0, 0, 3), abs=1e-14)
    assert surface.evaluate((1, 1))[0] == (0, 1, 3)


@pytest.mark.parametrize(
    "field,value",
    [
        ("degree", True),
        ("degree", 0),
        ("degree", 33),
        ("periodic", True),
        ("weights", [1, 0, 1]),
        ("weights", [1, -1, 1]),
        ("weights", [1, 1]),
        ("weights", [1, float("nan"), 1]),
        ("knots", [0, 0, 0, 1, 1]),
        ("knots", [0, 0, 0, 2, 1, 1]),
        ("knots", [-1, 0, 0, 1, 1, 2]),
    ],
)
def test_invalid_or_unsupported_definitions_are_rejected(tools, field, value):
    definition = quarter_circle() | {field: value}
    with pytest.raises(ValueError):
        tools["Nurbs"].read("curve", definition)


def test_sampling_includes_internal_knots_and_domain_endpoints(tools):
    definition = {
        "kind": "nurbs",
        "degree": 2,
        "periodic": False,
        "control_points": [{"x": i, "y": i % 2, "z": 0} for i in range(5)],
        "knots": [0, 0, 0, 0.2, 0.7, 1, 1, 1],
    }
    curve = tools["Nurbs"].read("curve", definition)
    assert len(curve.parameters(4)) == 13
    assert {(0,), (0.2,), (0.7,), (1,)} <= set(curve.parameters(4))
    for t in (0.2, 0.7):
        at = curve.evaluate((t,))
        for side in (-1, 1):
            near = curve.evaluate((t + side * 1e-10,))
            assert math.dist(at[0], near[0]) < 1e-7
            assert math.dist(at[1][0], near[1][0]) < 1e-7
    with pytest.raises(ValueError):
        curve.evaluate((1.1,))


def test_comparison_detects_pole_weight_and_tangent_corruption(tools):
    reference = tools["Nurbs"].read("curve", quarter_circle())
    for kind in ("pole", "weight"):
        damaged = quarter_circle()
        if kind == "pole":
            damaged["control_points"][1]["z"] = 0.01
        else:
            damaged["weights"][1] += 0.01
        comparison = tools["compare"](
            tools["Nurbs"].read("curve", damaged),
            reference,
            reference.evaluate,
            tolerances(),
            8,
        )
        assert not comparison["passed"]
        assert not comparison["checks"]["sampled_points_mm"]["passed"]

    def bad_tangent(params):
        p, (d,) = reference.evaluate(params)
        return p, (tuple(-x for x in d),)

    comparison = tools["compare"](reference, reference, bad_tangent, tolerances(), 8)
    assert comparison["checks"]["sampled_points_mm"]["passed"]
    assert not comparison["checks"]["sampled_derivatives"]["passed"]


def test_oracle_paths_and_binding_ids_fail_closed(tools, tmp_path):
    path = tmp_path / "source.bin"
    path.write_bytes(b"source")
    artifact = {
        "path": "source.bin",
        "byte_size": 6,
        "sha256": hashlib.sha256(b"source").hexdigest(),
    }
    assert tools["checked_file"](tmp_path, artifact) == path
    for changed in (
        artifact | {"byte_size": 5},
        artifact | {"sha256": "0" * 64},
        artifact | {"path": str(path)},
    ):
        with pytest.raises(ValueError):
            tools["checked_file"](tmp_path, changed)
    link = tmp_path / "escape"
    link.symlink_to(ROOT / "README.md")
    with pytest.raises(ValueError):
        tools["checked_file"](tmp_path, artifact | {"path": "escape"})
    with pytest.raises(ValueError):
        tools["unique"]([{"id": "same"}, {"id": "same"}])
    schema = json.loads(
        (ROOT / "docs/schemas/nurbs-geometry-oracle.schema.json").read_text()
    )
    Draft202012Validator.check_schema(schema)


def test_independent_ocp_rational_surface_derivatives(tools):
    pytest.importorskip("OCP")
    from OCP.Geom import Geom_BSplineSurface
    from OCP.gp import gp_Pnt
    from OCP.TColgp import TColgp_Array2OfPnt
    from OCP.TColStd import (
        TColStd_Array1OfInteger,
        TColStd_Array1OfReal,
        TColStd_Array2OfReal,
    )

    definition = quarter_cylinder()
    poles, weights = TColgp_Array2OfPnt(1, 3, 1, 2), TColStd_Array2OfReal(1, 3, 1, 2)
    for i in range(3):
        for j in range(2):
            p = definition["control_points"][i * 2 + j]
            poles.SetValue(i + 1, j + 1, gp_Pnt(p["x"], p["y"], p["z"]))
            weights.SetValue(i + 1, j + 1, definition["weights"][i * 2 + j])
    knots = TColStd_Array1OfReal(1, 2)
    knots.SetValue(1, 0)
    knots.SetValue(2, 1)
    um, vm = TColStd_Array1OfInteger(1, 2), TColStd_Array1OfInteger(1, 2)
    for i in (1, 2):
        um.SetValue(i, 3)
        vm.SetValue(i, 2)
    spline = Geom_BSplineSurface(
        poles, weights, knots, knots, um, vm, 2, 1, False, False
    )
    native = tools["Nurbs"].read("surface", definition)
    reference = tools["Nurbs"].read(
        "surface", tools["step_definition"]("surface", spline)
    )
    report = tools["compare"](
        native, reference, lambda p: tools["step_evaluate"](spline, p), tolerances(), 8
    )
    assert report["passed"]
    assert report["sample_count"] == 81
