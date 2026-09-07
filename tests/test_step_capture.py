from __future__ import annotations

import runpy
from pathlib import Path

import pytest

ROOT = Path(__file__).parents[1]


@pytest.fixture
def capture_tools():
    pytest.importorskip("OCP", reason="optional neutral reference capture environment")
    return runpy.run_path(str(ROOT / "scripts/capture_step_geometry.py"))


def test_step_body_inventory_does_not_double_count_owned_faces(capture_tools):
    from OCP.BRep import BRep_Builder
    from OCP.BRepBuilderAPI import BRepBuilderAPI_MakeFace
    from OCP.BRepPrimAPI import BRepPrimAPI_MakeBox
    from OCP.gp import gp_Dir, gp_Pln, gp_Pnt
    from OCP.TopoDS import TopoDS_Compound

    box = BRepPrimAPI_MakeBox(1, 2, 3).Shape()
    face = BRepBuilderAPI_MakeFace(
        gp_Pln(gp_Pnt(0, 0, 10), gp_Dir(0, 0, 1)), 0, 2, 0, 2
    ).Face()
    builder = BRep_Builder()
    compound = TopoDS_Compound()
    builder.MakeCompound(compound)
    builder.Add(compound, box)
    builder.Add(compound, face)
    bodies = capture_tools["_body_shapes"](compound)
    assert [kind for kind, _ in bodies] == ["solid", "sheet"]
    assert bodies[0][1].IsSame(box)
    assert bodies[1][1].IsSame(face)


def test_closed_free_shell_is_not_promoted_to_solid(capture_tools):
    from OCP.BRepPrimAPI import BRepPrimAPI_MakeBox
    from OCP.TopAbs import TopAbs_SHELL

    box = BRepPrimAPI_MakeBox(1, 2, 3).Shape()
    shell = capture_tools["_subshapes"](box, TopAbs_SHELL)[0]
    assert shell.Closed()
    assert [kind for kind, _ in capture_tools["_body_shapes"](shell)] == ["sheet"]


def test_step_capture_volume_excludes_open_sheet(capture_tools, tmp_path):
    from OCP.BRepBuilderAPI import BRepBuilderAPI_MakeFace
    from OCP.BRepPrimAPI import BRepPrimAPI_MakeBox
    from OCP.gp import gp_Dir, gp_Pln, gp_Pnt
    from OCP.IFSelect import IFSelect_RetDone
    from OCP.STEPControl import STEPControl_AsIs, STEPControl_Writer

    box = BRepPrimAPI_MakeBox(1, 2, 3).Shape()
    face = BRepBuilderAPI_MakeFace(
        gp_Pln(gp_Pnt(0, 0, 10), gp_Dir(0, 0, 1)), 0, 2, 0, 2
    ).Face()
    writer = STEPControl_Writer()
    assert writer.Transfer(box, STEPControl_AsIs) == IFSelect_RetDone
    assert writer.Transfer(face, STEPControl_AsIs) == IFSelect_RetDone
    path = tmp_path / "mixed.step"
    assert writer.Write(str(path)) == IFSelect_RetDone
    record = capture_tools["capture"](path)
    assert record["topology"]["bodies_by_kind"] == {"solid": 1, "sheet": 1}
    assert record["geometry"]["volume_mm3"] == pytest.approx(6)
    assert record["geometry"]["surface_area_mm2"] == pytest.approx(26)
    assert record["geometry"]["center_of_mass_mm"] == pytest.approx([0.5, 1, 1.5])
    assert record["carrier_counts"]["surfaces"] == {"GeomAbs_Plane": 7}
    assert record["capture"]["volume_scope"] == "explicit_solids_only"
