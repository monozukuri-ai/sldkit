#!/usr/bin/env python3
"""Capture independent B-Rep facts from neutral STEP files using OCP."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Any


def sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _subshape_count(shape: Any, kind: Any) -> int:
    return len(_subshapes(shape, kind))


def _subshapes(shape: Any, kind: Any) -> list[Any]:
    from OCP.TopExp import TopExp
    from OCP.TopTools import TopTools_IndexedMapOfShape

    shapes = TopTools_IndexedMapOfShape()
    TopExp.MapShapes_s(shape, kind, shapes)
    return [shapes.FindKey(index) for index in range(1, shapes.Extent() + 1)]


def _body_shapes(shape: Any) -> list[tuple[str, Any]]:
    """Keep solids, free shells and free faces separate; do not heal or sew."""
    from OCP.TopAbs import TopAbs_FACE, TopAbs_SHELL, TopAbs_SOLID

    solids = _subshapes(shape, TopAbs_SOLID)
    solid_shells = [
        shell for solid in solids for shell in _subshapes(solid, TopAbs_SHELL)
    ]
    shells = _subshapes(shape, TopAbs_SHELL)
    free_shells = [
        shell
        for shell in shells
        if not any(shell.IsSame(owned) for owned in solid_shells)
    ]
    shell_faces = [face for shell in shells for face in _subshapes(shell, TopAbs_FACE)]
    free_faces = [
        face
        for face in _subshapes(shape, TopAbs_FACE)
        if not any(face.IsSame(owned) for owned in shell_faces)
    ]
    return [
        *(("solid", solid) for solid in solids),
        *(("sheet", sheet) for sheet in [*free_shells, *free_faces]),
    ]


def _carrier_counts(shape: Any) -> dict[str, dict[str, int]]:
    from collections import Counter

    from OCP.BRepAdaptor import BRepAdaptor_Curve, BRepAdaptor_Surface
    from OCP.TopAbs import TopAbs_EDGE, TopAbs_FACE
    from OCP.TopoDS import TopoDS

    return {
        "curves": dict(
            sorted(
                Counter(
                    BRepAdaptor_Curve(TopoDS.Edge_s(edge)).GetType().name
                    for edge in _subshapes(shape, TopAbs_EDGE)
                ).items()
            )
        ),
        "surfaces": dict(
            sorted(
                Counter(
                    BRepAdaptor_Surface(TopoDS.Face_s(face)).GetType().name
                    for face in _subshapes(shape, TopAbs_FACE)
                ).items()
            )
        ),
    }


def capture(path: Path) -> dict[str, Any]:
    try:
        import OCP
        from OCP.Bnd import Bnd_Box
        from OCP.BRepBndLib import BRepBndLib
        from OCP.BRepCheck import BRepCheck_Analyzer
        from OCP.BRepGProp import BRepGProp
        from OCP.GProp import GProp_GProps
        from OCP.IFSelect import IFSelect_RetDone
        from OCP.STEPControl import STEPControl_Reader
        from OCP.TopAbs import (
            TopAbs_EDGE,
            TopAbs_FACE,
            TopAbs_SHELL,
            TopAbs_VERTEX,
        )
    except ImportError as error:
        raise RuntimeError(
            "OCP is required only for reference capture; run this script in an "
            "environment that provides the OCP package"
        ) from error

    source = path.resolve(strict=True)
    reader = STEPControl_Reader()
    if reader.ReadFile(str(source)) != IFSelect_RetDone:
        raise ValueError(f"STEP reader rejected {source}")
    transferred = reader.TransferRoots()
    if transferred <= 0:
        raise ValueError(f"STEP reader transferred no roots from {source}")
    shape = reader.OneShape()
    if shape.IsNull():
        raise ValueError(f"STEP reader returned a null shape for {source}")

    bodies = _body_shapes(shape)
    volume_properties = GProp_GProps()
    # Open sheets do not enclose a volume. Integrating them as if they did can
    # introduce a spurious volume and center of mass in mixed-body exports.
    for kind, body in bodies:
        if kind == "solid":
            properties = GProp_GProps()
            BRepGProp.VolumeProperties_s(body, properties)
            volume_properties.Add(properties)
    volume = float(volume_properties.Mass())
    center = None
    if abs(volume) > 1e-15:
        point = volume_properties.CentreOfMass()
        center = [float(point.X()), float(point.Y()), float(point.Z())]

    surface_properties = GProp_GProps()
    area_error = BRepGProp.SurfaceProperties_s(shape, surface_properties, 1e-9, False)

    box = Bnd_Box()
    BRepBndLib.AddOptimal_s(shape, box, False, False)
    bounds = [float(value) for value in box.Get()]

    body_kinds = {"solid": sum(kind == "solid" for kind, _ in bodies)}
    sheet_count = sum(kind == "sheet" for kind, _ in bodies)
    if sheet_count:
        body_kinds["sheet"] = sheet_count
    return {
        "path": source.name,
        "sha256": sha256_path(source),
        "capture": {
            "tool": "OCP STEPControl_Reader/BRepGProp",
            "tool_version": str(OCP.__version__),
            "length_unit": "millimeter",
            "body_classification_basis": "imported_brep_without_sewing",
            "volume_scope": "explicit_solids_only",
            "surface_integration_relative_tolerance": 1e-9,
            "surface_integration_estimated_relative_error": float(area_error),
        },
        "topology": {
            "bodies_by_kind": body_kinds,
            "shells": _subshape_count(shape, TopAbs_SHELL),
            "faces": _subshape_count(shape, TopAbs_FACE),
            "edges": _subshape_count(shape, TopAbs_EDGE),
            "vertices": _subshape_count(shape, TopAbs_VERTEX),
            "valid": bool(BRepCheck_Analyzer(shape).IsValid()),
        },
        "geometry": {
            "volume_mm3": abs(volume),
            "surface_area_mm2": float(surface_properties.Mass()),
            "center_of_mass_mm": center,
            "bounds_mm": bounds,
        },
        "carrier_counts": _carrier_counts(shape),
        "bodies": [
            {
                "kind": kind,
                "faces": _subshape_count(body, TopAbs_FACE),
                "edges": _subshape_count(body, TopAbs_EDGE),
                "vertices": _subshape_count(body, TopAbs_VERTEX),
                "carrier_counts": _carrier_counts(body),
            }
            for kind, body in bodies
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        records = [capture(path) for path in args.paths]
    except (OSError, RuntimeError, ValueError) as error:
        print(json.dumps({"schema_version": 1, "passed": False, "error": str(error)}))
        return 1
    output = (
        json.dumps(
            {"schema_version": 1, "records": records},
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    if args.output is None:
        print(output, end="")
    else:
        args.output.write_text(output, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
