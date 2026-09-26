"""Synthetic correspondence tests; no private CAD or claimed solid closure."""

import copy
import runpy
from pathlib import Path

import pytest

TOOL = runpy.run_path(
    str(Path(__file__).resolve().parents[1] / "scripts/validate_part_orientation.py")
)


def fixture():
    exact = {"exactness": "byte_exact"}
    model = {
        "bodies": [{"id": "body", "kind": "solid", "provenance": exact}],
        "regions": [{"id": "region", "provenance": exact}],
        "shells": [{"id": "shell", "provenance": exact}],
        "faces": [
            {
                "id": "face",
                "surface_id": "plane",
                "sense": "forward",
                "loop_ids": ["loop"],
            }
        ],
        "loops": [{"id": "loop", "coedge_ids": ["coedge"]}],
        "coedges": [
            {
                "id": "coedge",
                "edge_id": "edge",
                "sense": "forward",
                "provenance": {"tag": "00_11"},
            }
        ],
        "edges": [
            {
                "id": "edge",
                "curve_id": "circle",
                "start_vertex_id": "vertex",
                "end_vertex_id": "vertex",
            }
        ],
        "vertices": [{"id": "vertex", "point_id": "point"}],
        "points": [{"id": "point", "position": [2, 0, 0]}],
        "carriers": [
            {
                "id": "plane",
                "kind": "plane",
                "definition": {
                    "kind": "plane",
                    "origin": [0, 0, 0],
                    "normal": [0, 0, 1],
                },
            },
            {
                "id": "circle",
                "kind": "circle",
                "definition": {
                    "kind": "circle",
                    "center": [0, 0, 0],
                    "axis": [0, 0, 1],
                    "radius": 2,
                },
            },
        ],
    }
    faces = {(0, 0): {"opposite": False, "kind": "plane"}}
    groups = {
        (0, 0): [
            {
                "id": [0, 0, 0, 0],
                "a": [2, 0, 0],
                "b": [2, 0, 0],
                "p": [0, 2, 0],
                "t": [-1, 0, 0],
                "n": [0, 0, 1],
            }
        ]
    }
    return model, faces, groups


def test_orientation_detects_reversed_normal_and_curve_frame():
    model, faces, groups = fixture()
    assert TOOL["compare"](model, faces, groups, 1)["passed"]
    model["faces"][0]["sense"] = "reversed"
    result = TOOL["compare"](model, faces, groups, 1)
    assert not result["passed"] and not result["face_normals_passed"]
    assert result["coedge_tangents_passed"]
    faces[(0, 0)]["opposite"] = True
    model["carriers"][1]["definition"]["axis"] = [0, 0, -1]
    result = TOOL["compare"](model, faces, groups, 1)
    assert result["face_normals_passed"] and not result["coedge_tangents_passed"]
    model["coedges"][0]["sense"] = "reversed"
    assert TOOL["compare"](model, faces, groups, 1)["passed"]


def test_surface_support_offset_cannot_pass_on_normals_alone():
    model, faces, groups = fixture()
    model["carriers"][0]["definition"]["origin"][2] = 1
    with pytest.raises(ValueError, match="not unique"):
        TOOL["compare"](model, faces, groups, 1)


def test_correspondence_requires_all_faces_and_edges():
    model, faces, groups = fixture()
    with pytest.raises(ValueError, match="edge coverage"):
        TOOL["compare"](model, faces, groups, 2)
    faces[(0, 1)] = copy.deepcopy(faces[(0, 0)])
    groups[(0, 1)] = copy.deepcopy(groups[(0, 0)])
    with pytest.raises(ValueError, match="bijective"):
        TOOL["compare"](model, faces, groups, 1)


def test_api_capture_requires_complete_record_rosters(tmp_path):
    source = tmp_path / "part.SLDPRT"
    source.write_bytes(b"synthetic")
    capture = tmp_path / "capture.csv"
    capture.write_text(
        "CAPTURE,1,34.0.0\nSOURCE,part.SLDPRT,9\nUNITS,meter\nBODY,0,0,1,1\nCOMPLETE\n"
    )
    with pytest.raises(ValueError, match="face coverage"):
        TOOL["read_capture"](capture, source)
    capture.write_text("CAPTURE,1,34.0.0\n")
    with pytest.raises(ValueError, match="incomplete"):
        TOOL["read_capture"](capture, source)
