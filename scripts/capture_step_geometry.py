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
    from OCP.TopExp import TopExp
    from OCP.TopTools import TopTools_IndexedMapOfShape

    shapes = TopTools_IndexedMapOfShape()
    TopExp.MapShapes_s(shape, kind, shapes)
    return int(shapes.Extent())


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
            TopAbs_SOLID,
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

    volume_properties = GProp_GProps()
    BRepGProp.VolumeProperties_s(shape, volume_properties)
    volume = float(volume_properties.Mass())
    center = None
    if abs(volume) > 1e-15:
        point = volume_properties.CentreOfMass()
        center = [float(point.X()), float(point.Y()), float(point.Z())]

    surface_properties = GProp_GProps()
    BRepGProp.SurfaceProperties_s(shape, surface_properties)

    box = Bnd_Box()
    BRepBndLib.AddOptimal_s(shape, box, False, False)
    bounds = [float(value) for value in box.Get()]

    return {
        "path": source.name,
        "sha256": sha256_path(source),
        "capture": {
            "tool": "OCP STEPControl_Reader/BRepGProp",
            "tool_version": str(OCP.__version__),
            "length_unit": "millimeter",
        },
        "topology": {
            "bodies_by_kind": {
                "solid": _subshape_count(shape, TopAbs_SOLID),
            },
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
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()
    try:
        records = [capture(path) for path in args.paths]
    except (OSError, RuntimeError, ValueError) as error:
        print(json.dumps({"schema_version": 1, "passed": False, "error": str(error)}))
        return 1
    print(
        json.dumps(
            {"schema_version": 1, "records": records},
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
