from __future__ import annotations

import copy
import runpy
from pathlib import Path

ROOT = Path(__file__).parents[1]


def _namespace() -> dict:
    return runpy.run_path(str(ROOT / "scripts/validate_geometry_corpus.py"))


def _carrier(identity: str, domain: str, definition: dict) -> dict:
    return {
        "id": identity,
        "domain": domain,
        "kind": definition["kind"],
        "definition": definition,
    }


def test_nurbs_parameter_invariants_accept_curve_surface_and_polar_pcurve():
    namespace = _namespace()
    carriers = [
        _carrier(
            "curve",
            "curve",
            {
                "kind": "nurbs",
                "degree": 2,
                "knots": [0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                "control_points": [
                    {"x": 0.0, "y": 0.0, "z": 0.0},
                    {"x": 0.5, "y": 1.0, "z": 0.0},
                    {"x": 1.0, "y": 0.0, "z": 0.0},
                ],
                "weights": [1.0, 0.5, 1.0],
                "periodic": False,
            },
        ),
        _carrier(
            "surface",
            "surface",
            {
                "kind": "nurbs",
                "u_degree": 1,
                "v_degree": 1,
                "u_knots": [0.0, 0.0, 1.0, 1.0],
                "v_knots": [0.0, 0.0, 1.0, 1.0],
                "u_count": 2,
                "v_count": 2,
                "control_points": [
                    {"x": 0.0, "y": 0.0, "z": 0.0},
                    {"x": 0.0, "y": 1.0, "z": 0.0},
                    {"x": 1.0, "y": 0.0, "z": 0.0},
                    {"x": 1.0, "y": 1.0, "z": 0.0},
                ],
                "u_periodic": False,
                "v_periodic": False,
            },
        ),
        _carrier(
            "polar",
            "pcurve",
            {
                "kind": "polar_nurbs",
                "degree": 1,
                "knots": [0.0, 0.0, 1.0, 1.0],
                "radial_control_points": [
                    {"u": 1.0, "v": 0.0},
                    {"u": 0.0, "v": 1.0},
                ],
                "axial_control_points": [0.0, 1.0],
                "weights": [1.0, 1.0],
                "periodic": False,
            },
        ),
    ]

    assert namespace["carrier_parameter_errors"]({"carriers": carriers}) == []


def test_nurbs_parameter_invariants_reject_bad_knot_and_weight_counts():
    namespace = _namespace()
    carrier = _carrier(
        "bad-curve",
        "curve",
        {
            "kind": "nurbs",
            "degree": 2,
            "knots": [0.0, 0.0, 1.0],
            "control_points": [
                {"x": 0.0, "y": 0.0, "z": 0.0},
                {"x": 0.5, "y": 1.0, "z": 0.0},
                {"x": 1.0, "y": 0.0, "z": 0.0},
            ],
            "weights": [1.0],
            "periodic": False,
        },
    )
    reversed_knots = copy.deepcopy(carrier)
    reversed_knots["id"] = "reversed-curve"
    reversed_knots["definition"]["knots"] = [
        0.0,
        0.0,
        0.0,
        1.0,
        0.5,
        1.0,
    ]
    reversed_knots["definition"].pop("weights")

    errors = namespace["carrier_parameter_errors"](
        {"carriers": [carrier, reversed_knots]}
    )

    assert errors == [
        "bad-curve has inconsistent nurbs parameters",
        "reversed-curve has inconsistent nurbs parameters",
    ]
