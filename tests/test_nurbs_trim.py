from __future__ import annotations

import copy
import json
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).parents[1]


@pytest.fixture
def trim(monkeypatch):
    monkeypatch.syspath_prepend(str(ROOT / "scripts"))
    import validate_nurbs_trim

    return validate_nurbs_trim


def rectangle_model():
    corners = [(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (1.0, 1.0, 0.0), (0.0, 1.0, 0.0)]
    surface = {
        "kind": "nurbs",
        "u_degree": 1,
        "v_degree": 1,
        "u_count": 2,
        "v_count": 2,
        "u_periodic": False,
        "v_periodic": False,
        "u_knots": [0, 0, 1, 1],
        "v_knots": [0, 0, 1, 1],
        "control_points": [
            dict(zip("xyz", p, strict=True))
            for p in (corners[0], corners[3], corners[1], corners[2])
        ],
    }
    face = {
        "id": "face",
        "surface_id": "surface",
        "sense": "forward",
        "loop_ids": ["loop"],
    }
    model = {
        k: [] for k in ("loops", "coedges", "edges", "vertices", "points", "carriers")
    }
    model["carriers"].append({"id": "surface", "definition": surface})
    model["loops"].append(
        {
            "id": "loop",
            "face_id": "face",
            "coedge_ids": [f"c{i}" for i in range(4)],
            "boundary_role": "unspecified",
        }
    )
    for i, start in enumerate(corners):
        end = corners[(i + 1) % 4]
        direction = tuple(b - a for a, b in zip(start, end, strict=True))
        model["points"].append({"id": f"p{i}", "position": start})
        model["vertices"].append({"id": f"v{i}", "point_id": f"p{i}"})
        model["edges"].append(
            {
                "id": f"e{i}",
                "curve_id": f"curve{i}",
                "start_vertex_id": f"v{i}",
                "end_vertex_id": f"v{(i + 1) % 4}",
                "parameter_range": None,
            }
        )
        model["coedges"].append(
            {
                "id": f"c{i}",
                "loop_id": "loop",
                "edge_id": f"e{i}",
                "next_id": f"c{(i + 1) % 4}",
                "previous_id": f"c{(i - 1) % 4}",
                "sense": "forward",
                "pcurves": [{"pcurve_id": f"uv{i}", "parameter_range": None}],
            }
        )
        for identity, domain, keys in (
            (f"curve{i}", "curve", "xyz"),
            (f"uv{i}", "pcurve", "uv"),
        ):
            model["carriers"].append(
                {
                    "id": identity,
                    "domain": domain,
                    "definition": {
                        "kind": "line",
                        "origin": dict(zip(keys, start[: len(keys)], strict=True)),
                        "direction": dict(
                            zip(keys, direction[: len(keys)], strict=True)
                        ),
                    },
                    "pcurve_state": None,
                    "provenance": {
                        "tag": "derived_nurbs_isoparametric_pcurve",
                        "exactness": "derived",
                    },
                }
            )
    return model, face


def test_native_rectangle_retains_source_unknowns_and_closes(trim):
    model, face = rectangle_model()
    before = copy.deepcopy(model)
    result = trim.native_boundary(model, face, 1e-5, 1e-10)
    assert result["rectangle"]["signed_uv_area"] == 1
    assert result["rectangle"]["maximum_uv_closure_error"] == 0
    assert all(e["interval"] == [0, 1] for e in result["edges"])
    assert all(
        all(v is None for v in e["source_ranges"].values()) for e in result["edges"]
    )
    assert model == before


@pytest.mark.parametrize(
    "case",
    [
        "next",
        "previous",
        "owner",
        "vertex",
        "sense",
        "missing_pcurve",
        "pcurve_offset",
        "pcurve_direction",
        "wrapper",
        "source_range",
        "source_pcurve",
        "hole",
    ],
)
def test_invalid_native_topology_or_unsupported_metadata_fails(trim, case):
    model, face = rectangle_model()
    coedge = model["coedges"][0]
    pcurve = next(c for c in model["carriers"] if c["id"] == "uv0")
    if case in {"next", "previous"}:
        coedge[case + "_id"] = "c0"
    elif case == "owner":
        coedge["loop_id"] = "missing"
    elif case == "vertex":
        model["edges"][1]["start_vertex_id"] = "v3"
    elif case == "sense":
        coedge["sense"] = "unknown"
    elif case == "missing_pcurve":
        coedge["pcurves"] = []
    elif case == "pcurve_offset":
        pcurve["definition"]["origin"]["v"] = 0.1
    elif case == "pcurve_direction":
        pcurve["definition"]["direction"]["v"] = float("nan")
    elif case == "wrapper":
        pcurve["pcurve_state"] = {"wrapper_reversed": True}
    elif case == "source_range":
        model["edges"][0]["parameter_range"] = [0, 1]
    elif case == "source_pcurve":
        pcurve["provenance"]["tag"] = "unverified_source_record"
    elif case == "hole":
        face["loop_ids"].append("another_loop")
    with pytest.raises(ValueError):
        trim.native_boundary(model, face, 1e-5, 1e-10)


def test_line_parameters_respect_nonunit_direction_and_vertex_residual(trim):
    definition = {
        "kind": "line",
        "origin": {"x": 1, "y": 0, "z": 0},
        "direction": {"x": 2, "y": 0, "z": 0},
    }
    evaluate, interval, axis = trim.curve_interval(
        definition, [(7, 0, 0), (3, 0, 0)], 1e-5
    )
    assert interval == [3, 1] and axis is None
    assert evaluate(2) == ((5, 0, 0), (2, 0, 0))
    with pytest.raises(ValueError, match="vertex"):
        trim.curve_interval(definition, [(7, 0.01, 0), (3, 0, 0)], 1e-5)


def test_nurbs_interval_uses_full_support_and_rejects_partial_or_ambiguous(trim):
    definition = {
        "kind": "nurbs",
        "degree": 1,
        "periodic": False,
        "knots": [2, 2, 5, 5],
        "control_points": [{"x": 0, "y": 0, "z": 0}, {"x": 1, "y": 0, "z": 0}],
    }
    _, interval, _ = trim.curve_interval(definition, [(1, 0, 0), (0, 0, 0)], 1e-5)
    assert interval == [5, 2]
    for endpoints in ([(0.2, 0, 0), (1, 0, 0)], [(0, 0, 0), (0, 0, 0)]):
        with pytest.raises(ValueError):
            trim.curve_interval(definition, endpoints, 1e-5)


def test_rectangle_rejects_duplicate_sides_and_open_traversal(trim):
    corners = [(0, 0), (1, 0), (1, 1), (0, 1)]
    segments = [(corners[i], corners[(i + 1) % 4]) for i in range(4)]
    assert (
        trim.rectangle_check(segments, [(0, 1), (0, 1)], 1e-10)["signed_uv_area"] == 1
    )
    for altered in (
        [segments[0], segments[1], segments[2], segments[0]],
        [segments[0], segments[2], segments[1], segments[3]],
    ):
        with pytest.raises(ValueError):
            trim.rectangle_check(altered, [(0, 1), (0, 1)], 1e-10)
    assert trim.cyclic_equal([3, 4, 1, 2], [1, 2, 3, 4])
    assert not trim.cyclic_equal([1, 4, 3, 2], [1, 2, 3, 4])


def test_uv_roundoff_clamp_does_not_accept_outside_trim(trim):
    model, face = rectangle_model()
    surface = trim.native_boundary(model, face, 1e-5, 1e-10)["surface"]
    assert trim.lift(surface, (-1e-16, 0.5), (0, 1), 1e-10)[0] == (0, 0.5, 0)
    with pytest.raises(ValueError, match="outside"):
        trim.lift(surface, (-1e-8, 0.5), (0, 1), 1e-10)


@pytest.mark.parametrize("reverse", [False, True])
@pytest.mark.parametrize("damage", [None, "curve", "lift", "tangent"])
def test_geometry_and_orientation_are_independent_gates(
    trim, monkeypatch, reverse, damage
):
    model, face = rectangle_model()
    native = trim.native_boundary(model, face, 1e-5, 1e-10)
    edge = native["edges"][0]

    # Independent analytic plane reference. Same locus, optionally reversed use.
    def step_curve(t):
        return (t, 0, 0.01 * t * (1 - t) if damage == "curve" else 0), (
            1,
            0,
            0.1 if damage == "tangent" else 0,
        )

    def step_surface(_, uv):
        return (uv[0], uv[1], 0.01 if damage == "lift" else 0), ((1, 0, 0), (0, 1, 0))

    monkeypatch.setattr(trim.support, "step_evaluate", step_surface)
    reference = {
        "index": 1,
        "interval": [1, 0] if reverse else [0, 1],
        "evaluate": step_curve,
        "uv_evaluate": lambda t: ((t, 0), (1, 0)),
        "endpoints": [(1, 0, 0), (0, 0, 0)] if reverse else [(0, 0, 0), (1, 0, 0)],
    }
    report = trim.compare_edge(
        edge,
        reference,
        native["surface"],
        None,
        {
            "position_mm": 1e-5,
            "derivative_absolute_mm": 1e-7,
            "derivative_relative": 1e-9,
        },
        1e-10,
        16,
    )
    assert report["derived_boundary_gate_passed"] == (damage is None)
    assert report["oriented_edge_gate_passed"] == (not reverse)
    assert report["traversal_relation"] == ("reversed" if reverse else "same")


def test_trim_oracle_schema_rejects_missing_or_extra_bindings():
    schema = json.loads(
        (ROOT / "docs/schemas/nurbs-trim-oracle.schema.json").read_text()
    )
    Draft202012Validator.check_schema(schema)
    binding = {
        "native_face_id": "f",
        "step_face_index": 1,
        "coedges": [
            {"native_coedge_id": str(i), "step_edge_index": i + 1} for i in range(4)
        ],
    }
    oracle = {
        "schema_version": 1,
        "profile": "rectangular_isoparametric_trim_v1",
        "support_oracle": {"path": "support.json", "byte_size": 1, "sha256": "0" * 64},
        "uv_tolerance": 1e-10,
        "face_bindings": [binding],
    }
    validator = Draft202012Validator(schema)
    validator.validate(oracle)
    binding["coedges"].pop()
    assert list(validator.iter_errors(oracle))
