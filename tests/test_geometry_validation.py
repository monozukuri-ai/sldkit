from __future__ import annotations

import copy
import hashlib
import runpy
from pathlib import Path

ROOT = Path(__file__).parents[1]


def _partition_fixture():
    body = b"0123456789"
    spans = [
        {
            "domain_id": "d",
            "offset": a,
            "byte_len": b - a,
            "classification": "typed",
            "tag": "field",
            "sha256": hashlib.sha256(body[a:b]).hexdigest(),
        }
        for a, b in [(2, 5), (4, 7)]
    ]
    ranges = [
        {
            "domain_id": "d",
            "offset": a,
            "byte_len": b - a,
            "classification": c,
            "reason": "reader",
        }
        for a, b, c in [
            (0, 2, "uninterpreted"),
            (2, 7, "typed"),
            (7, 10, "uninterpreted"),
        ]
    ]
    geometry = {
        "fidelity": {
            "byte_domains": [{"id": "d", "byte_len": 10}],
            "byte_spans": spans,
            "byte_ranges": ranges,
            "byte_coverage": {
                "partition_status": "complete",
                "typed_bytes": 5,
                "uninterpreted_bytes": 5,
                "partition_domain_bytes": 10,
                "classified_active_bytes": 10,
                "unclassified_active_bytes": 0,
            },
        }
    }
    return geometry, {"d": body}


def test_byte_partition_independently_checks_union_complement_and_hashes():
    validate = _namespace()["byte_partition_errors"]
    geometry, bodies = _partition_fixture()
    assert validate(geometry, bodies) == []
    for field, value in [
        ("offset", 3),
        ("byte_len", 0),
        ("sha256", "bad"),
        ("domain_id", "missing"),
    ]:
        damaged = copy.deepcopy(geometry)
        damaged["fidelity"]["byte_spans"][0][field] = value
        assert validate(damaged, bodies), field
    for field, value in [("offset", 1), ("byte_len", 4), ("classification", "typed")]:
        damaged = copy.deepcopy(geometry)
        damaged["fidelity"]["byte_ranges"][0][field] = value
        assert validate(damaged, bodies), field
    damaged = copy.deepcopy(geometry)
    damaged["fidelity"]["byte_spans"][1]["classification"] = "uninterpreted"
    assert validate(damaged, bodies)


def test_byte_partition_public_models_round_trip_and_accept_older_fidelity():
    import sldkit

    geometry, _ = _partition_fixture()
    span = geometry["fidelity"]["byte_spans"][0] | {"source_record_id": 42}
    interval = geometry["fidelity"]["byte_ranges"][0]
    assert sldkit.GeometryDecodedSpan.from_dict(span).to_dict() == span
    assert sldkit.GeometryByteRange.from_dict(interval).to_dict() == interval
    old = {
        "decoder": "test",
        "decoder_version": "0",
        "geometry_transferred": False,
        "entity_counts": {},
        "losses": [],
        "validation_findings": [],
        "byte_coverage": {
            "source_bytes": 0,
            "candidate_stream_bytes": 0,
            "active_stream_bytes": 0,
            "partition_domain_bytes": 0,
            "retained_record_bytes": 0,
            "located_entity_count": 0,
            "unique_location_count": 0,
            "classified_active_bytes": 0,
            "unclassified_active_bytes": 0,
            "partition_status": "incomplete",
            "typed_bytes": None,
            "uninterpreted_bytes": None,
        },
    }
    model = sldkit.GeometryFidelityReport.from_dict(old)
    assert model.byte_spans == model.byte_ranges == ()
    assert model.to_dict() == old


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
