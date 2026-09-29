"""Synthetic negative controls for the private real-file boundary oracle."""

import math
import runpy
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
TOOL = runpy.run_path(str(ROOT / "scripts/validate_part_boundaries.py"))
ORIENTATION = runpy.run_path(str(ROOT / "tests/test_part_orientation.py"))


def fixture():
    model, faces, groups = ORIENTATION["fixture"]()
    model["carriers"][0]["definition"]["u_axis"] = [1, 0, 0]
    model["carriers"][1]["definition"]["ref_direction"] = [1, 0, 0]
    model["edges"][0]["derived_parameter_interval"] = {
        "parameter_range": [0, math.tau],
    }
    model["loops"][0].update(
        face_id="face",
        boundary_role="unspecified",
        derived_boundary_role={"role": "outer", "signed_area_mm2": 4 * math.pi},
    )
    model["coedges"][0].update(
        next_id="coedge", previous_id="coedge", pcurves=[{"pcurve_id": "pcurve"}]
    )
    model["carriers"].append(
        {
            "id": "pcurve",
            "kind": "circle",
            "definition": {
                "kind": "circle",
                "center": [0, 0],
                "x_axis": [1, 0],
                "y_axis": [0, 1],
                "radius": 2,
            },
        }
    )
    row = ["0"] * 35
    row[0] = "COEDGE"
    row[13] = str(math.tau)
    rows = [
        ["FACE", "0", "0", "0", str(4 * math.pi / 1e6), "plane"],
        ["LOOP", "0", "0", "0", "-1", "1"],
        row,
    ]
    orientation = ORIENTATION["TOOL"]["compare"](model, faces, groups, 1)
    return model, orientation, rows


def test_full_circle_seam_gauge_and_plane_boundary():
    result = TOOL["validate"](*fixture())
    assert result["passed"]
    assert result["planar_roles"] == {"outer": 1}
    assert result["pcurve_samples"] == 17
    assert not result["source_trim_verified"]


@pytest.mark.parametrize("end", [math.pi, 2 * math.tau])
def test_matching_circle_endpoints_do_not_hide_wrong_arc_extent(end):
    model, orientation, rows = fixture()
    model["edges"][0]["derived_parameter_interval"]["parameter_range"][1] = end
    assert not TOOL["validate"](model, orientation, rows)["passed"]


@pytest.mark.parametrize("damage", ["role", "area", "pcurve", "surface"])
def test_boundary_oracle_rejects_geometric_and_semantic_damage(damage):
    model, orientation, rows = fixture()
    if damage == "role":
        model["loops"][0]["derived_boundary_role"]["role"] = "inner"
    elif damage == "area":
        model["loops"][0]["derived_boundary_role"]["signed_area_mm2"] *= -1
    elif damage == "pcurve":
        model["carriers"][-1]["definition"]["radius"] = 3
    else:
        model["carriers"][0]["definition"]["origin"][2] = 1
    assert not TOOL["validate"](model, orientation, rows)["passed"]


def test_boundary_oracle_rejects_broken_rings_and_nonfinite_intervals():
    model, orientation, rows = fixture()
    model["coedges"][0]["next_id"] = "missing"
    with pytest.raises(ValueError, match="ring order"):
        TOOL["validate"](model, orientation, rows)
    model, orientation, rows = fixture()
    model["edges"][0]["derived_parameter_interval"]["parameter_range"][1] = math.nan
    with pytest.raises(ValueError, match="invalid effective edge interval"):
        TOOL["validate"](model, orientation, rows)


def test_step_cylinder_groups_reject_area_and_axis_damage(tmp_path):
    pytest.importorskip("OCP")
    from OCP.BRepPrimAPI import BRepPrimAPI_MakeCylinder
    from OCP.IFSelect import IFSelect_RetDone
    from OCP.STEPControl import STEPControl_AsIs, STEPControl_Writer

    destination = tmp_path / "cylinder.step"
    writer = STEPControl_Writer()
    assert (
        writer.Transfer(BRepPrimAPI_MakeCylinder(2, 3).Shape(), STEPControl_AsIs)
        == IFSelect_RetDone
    )
    assert writer.Write(str(destination)) == IFSelect_RetDone
    model = {
        "carriers": [
            {
                "id": "surface",
                "definition": {
                    "kind": "cylinder",
                    "origin": [0, 0, 0],
                    "axis": [0, 0, 1],
                    "radius": 2,
                },
            }
        ],
        "faces": [{"id": "face", "surface_id": "surface", "loop_ids": ["loop"]}],
        "loops": [
            {"id": "loop", "derived_boundary_role": {"signed_area_mm2": 12 * math.pi}}
        ],
    }
    assert TOOL["step_areas"](model, destination)["passed"]
    model["loops"][0]["derived_boundary_role"]["signed_area_mm2"] += 0.01
    assert not TOOL["step_areas"](model, destination)["passed"]
    model["carriers"][0]["definition"]["axis"] = [1, 0, 0]
    with pytest.raises(ValueError, match="unmatched cylinder"):
        TOOL["step_areas"](model, destination)


def test_cylinder_hole_reference_area_with_independent_ocp(tmp_path):
    """Independent reference for the Rust public-mapping cylinder-hole test."""
    pytest.importorskip("OCP")
    from OCP.BRepBuilderAPI import (
        BRepBuilderAPI_MakeEdge,
        BRepBuilderAPI_MakeFace,
        BRepBuilderAPI_MakeWire,
    )
    from OCP.BRepCheck import BRepCheck_Analyzer
    from OCP.BRepGProp import BRepGProp
    from OCP.BRepLib import BRepLib
    from OCP.GCE2d import GCE2d_MakeSegment
    from OCP.Geom import Geom_CylindricalSurface
    from OCP.gp import gp_Ax3, gp_Dir, gp_Pnt, gp_Pnt2d
    from OCP.GProp import GProp_GProps
    from OCP.IFSelect import IFSelect_RetDone
    from OCP.STEPControl import STEPControl_AsIs, STEPControl_Writer

    surface = Geom_CylindricalSurface(gp_Ax3(gp_Pnt(), gp_Dir(0, 0, 1)), 2)

    def wire(low, high, inner=False):
        points = [low, (high[0], low[1]), high, (low[0], high[1])]
        if inner:
            points.reverse()
        builder = BRepBuilderAPI_MakeWire()
        for a, b in zip(points, points[1:] + points[:1], strict=True):
            segment = GCE2d_MakeSegment(gp_Pnt2d(*a), gp_Pnt2d(*b)).Value()
            builder.Add(BRepBuilderAPI_MakeEdge(segment, surface).Edge())
        return builder.Wire()

    builder = BRepBuilderAPI_MakeFace(surface, wire((0, 0), (5, 10)), True)
    builder.Add(wire((1, 1), (2, 3), inner=True))
    shape = builder.Face()
    assert BRepLib.BuildCurves3d_s(shape)
    assert BRepCheck_Analyzer(shape).IsValid()
    properties = GProp_GProps()
    BRepGProp.SurfaceProperties_s(shape, properties, 1e-12)
    assert properties.Mass() == pytest.approx(96, abs=1e-10)

    destination = tmp_path / "cylinder-hole.step"
    writer = STEPControl_Writer()
    assert writer.Transfer(shape, STEPControl_AsIs) == IFSelect_RetDone
    assert writer.Write(str(destination)) == IFSelect_RetDone
    model = {
        "carriers": [
            {
                "id": "surface",
                "definition": {
                    "kind": "cylinder",
                    "origin": [0, 0, 0],
                    "axis": [0, 0, 1],
                    "radius": 2,
                },
            }
        ],
        "faces": [
            {"id": "face", "surface_id": "surface", "loop_ids": ["hole", "outer"]}
        ],
        "loops": [
            {"id": "outer", "derived_boundary_role": {"signed_area_mm2": 100}},
            {"id": "hole", "derived_boundary_role": {"signed_area_mm2": -4}},
        ],
    }
    assert TOOL["step_areas"](model, destination)["passed"]
    model["loops"][1]["derived_boundary_role"]["signed_area_mm2"] = 4
    assert not TOOL["step_areas"](model, destination)["passed"]
