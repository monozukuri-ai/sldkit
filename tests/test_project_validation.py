from __future__ import annotations

import runpy
from pathlib import Path

VALIDATOR = runpy.run_path(
    str(Path(__file__).parents[1] / "scripts" / "validate_project_graph.py")
)
closure_difference = VALIDATOR["closure_difference"]
contains_path_fields = VALIDATOR["contains_path_fields"]


def test_closure_difference_reports_exact_and_unresolved_mismatches():
    graph = {
        "status": "partial",
        "nodes": [{"path": "root.SLDASM"}, {"path": "parts/child.SLDPRT"}],
        "edges": [
            {"id": "edge:0", "resolution_status": "resolved"},
            {"id": "edge:1", "resolution_status": "missing"},
        ],
    }

    difference = closure_difference(
        graph, ["root.sldasm", "parts/expected.SLDPRT"]
    )

    assert difference["status"] == "mismatch"
    assert difference["missing_from_graph"] == ["parts/expected.SLDPRT"]
    assert difference["unexpected_in_graph"] == ["parts/child.SLDPRT"]
    assert difference["unresolved_edges"] == [
        {"edge_id": "edge:1", "resolution_status": "missing"}
    ]


def test_closure_difference_rejects_case_colliding_expected_labels():
    graph = {
        "status": "complete",
        "nodes": [{"path": "root.SLDASM"}],
        "edges": [],
    }

    difference = closure_difference(graph, ["root.SLDASM", "ROOT.sldasm"])

    assert difference["status"] == "mismatch"
    assert difference["expected_case_collisions"] == [
        ["ROOT.sldasm", "root.SLDASM"]
    ]


def test_closure_difference_accepts_a_complete_exact_document_set():
    graph = {
        "status": "complete",
        "nodes": [{"path": "root.SLDASM"}, {"path": "parts/child.SLDPRT"}],
        "edges": [{"id": "edge:0", "resolution_status": "resolved"}],
    }

    difference = closure_difference(
        graph, ["ROOT.sldasm", "parts/CHILD.sldprt"]
    )

    assert difference["status"] == "match"


def test_compatibility_report_path_field_guard_is_recursive():
    assert not contains_path_fields({"counts": {"resolved": 2}})
    assert contains_path_fields({"nested": [{"stored_path": "secret.SLDPRT"}]})
