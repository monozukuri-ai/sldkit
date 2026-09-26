"""Negative controls for the saved/reopened NURBS API oracle."""

import csv
import runpy
from pathlib import Path

import pytest


@pytest.fixture
def fixture(tmp_path, monkeypatch):
    root = Path(__file__).resolve().parents[1]
    monkeypatch.syspath_prepend(str(root / "scripts"))
    tool = runpy.run_path(str(root / "scripts/validate_partial_nurbs.py"))
    source = tmp_path / "fixture.SLDPRT"
    source.write_bytes(b"synthetic")
    positions = [[0, 0, 0], [2, 0, 0], [2, 2, 0], [0, 2, 0]]
    model = {
        "bodies": [{}],
        "faces": [{}],
        "edges": [],
        "carriers": [],
        "vertices": [],
        "points": [],
    }
    rows = [
        ["CAPTURE", 2, "synthetic"],
        ["SOURCE", source.name, 9],
        ["UNITS", "meter"],
        ["BODY", 0, 1, 1, 4],
        ["FACE", 0, 0, 0, 0.000004, 0, 1, 0, 1],
    ]
    for i, a in enumerate(positions):
        b = positions[(i + 1) % 4]
        delta = [y - x for x, y in zip(a, b, strict=True)]
        poles = [
            [x - d / 2 for x, d in zip(a, delta, strict=True)],
            [x + d / 2 for x, d in zip(b, delta, strict=True)],
        ]
        model["points"].append({"id": f"p{i}", "position": a})
        model["vertices"].append({"id": f"v{i}", "point_id": f"p{i}"})
        model["carriers"].append(
            {
                "id": f"c{i}",
                "definition": {
                    "kind": "nurbs",
                    "degree": 1,
                    "knots": [0, 0, 1, 1],
                    "periodic": False,
                    "control_points": [dict(zip("xyz", p, strict=True)) for p in poles],
                },
            }
        )
        model["edges"].append(
            {
                "id": f"e{i}",
                "curve_id": f"c{i}",
                "start_vertex_id": f"v{i}",
                "end_vertex_id": f"v{(i + 1) % 4}",
                "derived_parameter_interval": {
                    "parameter_range": [0.25, 0.75],
                    "method": "nurbs_monotone_projection",
                },
            }
        )
        rows.append(
            [
                "EDGE",
                0,
                i,
                3006,
                -1,
                10,
                20,
                *[x / 1000 for x in a],
                *[x / 1000 for x in b],
            ]
        )
        rows.append(["SUPPORT", 0, i, -1, 5, 25, 0, 0])
        for j in range(17):
            rows.append(
                [
                    "SAMPLE",
                    0,
                    i,
                    j,
                    10 + 10 * j / 16,
                    *[(x + d * j / 16) / 1000 for x, d in zip(a, delta, strict=True)],
                    *[d / 10000 for d in delta],
                    0,
                ]
            )
    rows.append(["COMPLETE"])
    capture = tmp_path / "api.csv"
    with capture.open("w", newline="") as stream:
        csv.writer(stream).writerows(rows)
    return tool, model, capture, source


def test_partial_intervals_and_affine_parameter_gauges(fixture):
    tool, model, path, source = fixture
    result = tool["validate"](model, tool["read_capture"](path, source))
    assert result["passed"]
    assert result["samples"] == 68
    assert result["partial_support_intervals"] == 4
    assert not result["source_trim_verified"]


@pytest.mark.parametrize("damage", ["interval", "sample", "tangent"])
def test_oracle_rejects_wrong_intervals_positions_and_tangents(fixture, damage):
    tool, model, path, source = fixture
    capture = tool["read_capture"](path, source)
    if damage == "interval":
        model["edges"][0]["derived_parameter_interval"]["parameter_range"].reverse()
    elif damage == "sample":
        capture[1][(0, 0)][8][2] += 1e-4
    else:
        capture[1][(0, 0)][8][4] *= -1
    assert not tool["validate"](model, capture)["passed"]


def test_oracle_rejects_incomplete_capture(fixture):
    tool, _, path, source = fixture
    text = path.read_text()
    path.write_text(text.replace("COMPLETE", ""))
    with pytest.raises(ValueError, match="completion"):
        tool["read_capture"](path, source)
