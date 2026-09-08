from __future__ import annotations

import base64
import html
import json
import math
import re
import stat
import struct
from importlib.resources import files
from pathlib import Path
from typing import Any

from ..api import decode_geometry_file, extract_resource_file, parse_file
from ..model import (
    BinaryResourceKind,
    DocumentKind,
    GeometryResult,
    GeometryTessellation,
    LimitProfile,
    ParseResult,
)

# Display budgets are independent of parser limits and never change its results.
MAX_VERTICES = 2_000_000
MAX_TRIANGLES = 2_000_000
MAX_PREVIEWS = 64
MAX_PREVIEW_BYTES = 32 * 1024 * 1024
MAX_PREVIEW_PIXELS = 16_777_216
MAX_JSON_BYTES = 128 * 1024 * 1024


def _vectors(values: Any, width: int) -> bool:
    return all(
        len(row) == width and all(math.isfinite(v) and abs(v) <= 1e30 for v in row)
        for row in values
    )


def _indices(values: Any, width: int, size: int) -> bool:
    return all(
        len(row) == width and all(type(i) is int and 0 <= i < size for i in row)
        for row in values
    )


def _mesh(mesh: GeometryTessellation, warnings: list[str]) -> dict[str, Any] | None:
    if not mesh.vertices or not mesh.triangles:
        warnings.append(f"{mesh.id}: no triangle mesh available.")
        return None
    if not _vectors(mesh.vertices, 3) or not _indices(
        mesh.triangles, 3, len(mesh.vertices)
    ):
        warnings.append(f"{mesh.id}: invalid coordinates or triangle indices; omitted.")
        return None
    normals = mesh.normals
    corners = mesh.corner_normals
    edges = mesh.feature_edges
    if normals and (len(normals) != len(mesh.vertices) or not _vectors(normals, 3)):
        warnings.append(f"{mesh.id}: invalid vertex normals; omitted.")
        normals = ()
    if corners and (
        len(corners) != 3 * len(mesh.triangles) or not _vectors(corners, 3)
    ):
        warnings.append(f"{mesh.id}: invalid corner normals; omitted.")
        corners = ()
    if not _indices(edges, 2, len(mesh.vertices)):
        warnings.append(f"{mesh.id}: invalid feature edges; omitted.")
        edges = ()
    source = mesh.source_object
    color = source.color if source else None
    if color is not None and not all(math.isfinite(v) and 0 <= v <= 1 for v in color):
        warnings.append(f"{mesh.id}: invalid color; using display default.")
        color = None
    return {
        "id": mesh.id,
        "name": source.name if source else None,
        "body_id": mesh.body_id,
        "face_ids": mesh.face_ids,
        "vertices": mesh.vertices,
        "triangles": mesh.triangles,
        "normals": normals,
        "corner_normals": corners,
        "feature_edges": edges,
        "color": color,
        "visible": source.visible if source else None,
        "exactness": mesh.provenance.exactness.value,
    }


def _scene(result: GeometryResult | None, title: str) -> dict[str, Any]:
    geometry = result.geometry if result else None
    warnings: list[str] = []
    meshes = []
    vertices = triangles = 0
    if geometry:
        for mesh in geometry.model.tessellations:
            if (
                vertices + len(mesh.vertices) > MAX_VERTICES
                or triangles + len(mesh.triangles) > MAX_TRIANGLES
            ):
                warnings.append(f"{mesh.id}: viewer mesh budget exceeded; omitted.")
                continue
            converted = _mesh(mesh, warnings)
            if converted is not None:
                meshes.append(converted)
                vertices += len(mesh.vertices)
                triangles += len(mesh.triangles)
    return {
        "schema_version": 1,
        "title": title,
        "parse_status": None,
        "geometry_status": result.status.value if result else "not_requested",
        "unit": geometry.length_unit if geometry else None,
        "meshes": meshes,
        "counts": {"meshes": len(meshes), "vertices": vertices, "triangles": triangles},
        "body_count": len(geometry.model.bodies) if geometry else 0,
        "body_ids": [body.id for body in geometry.model.bodies] if geometry else [],
        "face_count": len(geometry.model.faces) if geometry else 0,
        "configurations": [c.to_dict() for c in geometry.configurations]
        if geometry
        else [],
        "diagnostics": [d.to_dict() for d in result.diagnostics] if result else [],
        "losses": [loss.to_dict() for loss in geometry.fidelity.losses]
        if geometry
        else [],
        "warnings": warnings,
        "previews": [],
        "document": None,
    }


def _image_data(data: bytes, kind: BinaryResourceKind) -> tuple[str, bytes]:
    if kind is BinaryResourceKind.PREVIEW_PNG:
        if len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
            raise ValueError("invalid PNG header")
        width, height = struct.unpack_from(">II", data, 16)
        media = "image/png"
    elif kind is BinaryResourceKind.PREVIEW_DIB:
        # Add a BMP file header; the browser decodes pixels, including RLE.
        if len(data) < 40:
            raise ValueError("truncated DIB header")
        header, width, height, planes, depth, compression = struct.unpack_from(
            "<IiiHHI", data
        )
        if (
            header not in (40, 108, 124)
            or header > len(data)
            or planes != 1
            or depth not in (1, 4, 8, 16, 24, 32)
            or compression not in (0, 1, 2, 3)
            or (compression == 1 and depth != 8)
            or (compression == 2 and depth != 4)
            or (compression in (1, 2) and height <= 0)
            or (compression == 3 and depth not in (16, 32))
            or (header == 124 and any(struct.unpack_from("<II", data, 112)))
        ):
            raise ValueError("unsupported DIB layout")
        height = abs(height)
        colors = struct.unpack_from("<I", data, 32)[0]
        if depth <= 8:
            if colors > 1 << depth:
                raise ValueError("invalid DIB palette")
            colors = colors or 1 << depth
        offset = header + colors * 4 + (12 if header == 40 and compression == 3 else 0)
        pixel_bytes = ((width * depth + 31) // 32) * 4 * height
        if compression in (1, 2):
            pixel_bytes = struct.unpack_from("<I", data, 20)[0]
            if pixel_bytes == 0:
                raise ValueError("RLE DIB requires a declared image size")
        if offset + pixel_bytes > len(data):
            raise ValueError("truncated DIB pixels")
        data = struct.pack("<2sIHHI", b"BM", len(data) + 14, 0, 0, offset + 14) + data
        media = "image/bmp"
    else:
        raise ValueError("unsupported preview format")
    if width <= 0 or height <= 0 or width * height > MAX_PREVIEW_PIXELS:
        raise ValueError("preview dimensions exceed viewer limits")
    return media, data


def _document(scene: dict[str, Any], result: ParseResult) -> list[tuple[str, Any]]:
    scene["parse_status"] = result.status.value
    scene["diagnostics"].extend(d.to_dict() for d in result.diagnostics)
    doc = result.document
    if doc is None:
        return []

    def value(item: Any) -> Any:
        return item.value if item is not None else None

    scene["document"] = {
        "kind": doc.document_kind.value.value,
        "configurations": [value(c.name) for c in doc.configurations],
        "references": [
            {
                "name": value(r.source_name),
                "path": value(r.stored_path),
                "configuration": value(r.configuration),
            }
            for r in doc.references
        ],
        "sheets": [
            {
                "name": value(s.name),
                "views": [
                    {"name": value(v.name), "document": value(v.referenced_document)}
                    for v in s.views
                ],
            }
            for s in doc.sheets
        ],
    }
    resources = [("Document", doc.preview)]
    resources.extend(
        (f"Configuration: {value(c.name) or value(c.index)}", c.preview)
        for c in doc.configurations
    )
    resources.extend(
        (f"Sheet: {value(s.name) or i + 1}", s.preview)
        for i, s in enumerate(doc.sheets)
    )
    return [(name, resource) for name, resource in resources if resource is not None]


def _write(scene: dict[str, Any], output: Path, force: bool) -> Path:
    assets = files("sldkit.viewer").joinpath("_assets")
    payload = json.dumps(
        scene, ensure_ascii=True, allow_nan=False, separators=(",", ":")
    )
    if len(payload) > MAX_JSON_BYTES:
        raise ValueError("viewer JSON exceeds 128 MiB; select a smaller input")
    payload = (
        payload.replace("<", "\\u003c").replace(">", "\\u003e").replace("&", "\\u0026")
    )
    replacements = {
        "TITLE": html.escape(scene["title"]),
        "STYLE": assets.joinpath("viewer.css").read_text(encoding="utf-8"),
        "DATA": payload,
        "SCRIPT": assets.joinpath("viewer.js").read_text(encoding="utf-8"),
        "LICENSE": html.escape(assets.joinpath("three-LICENSE.txt").read_text("utf-8")),
    }
    template = assets.joinpath("viewer.html").read_text(encoding="utf-8")
    # Replace only template tokens, never tokens inside untrusted source strings.
    rendered = re.sub(
        r"@@(TITLE|STYLE|DATA|SCRIPT|LICENSE)@@", lambda m: replacements[m[1]], template
    )
    with output.open("w" if force else "x", encoding="utf-8") as destination:
        destination.write(rendered)
    return output


def _output_path(output: str | Path) -> Path:
    path = Path(output)
    if path.suffix.lower() not in (".html", ".htm"):
        raise ValueError("viewer output must use .html or .htm")
    return path


def write_html(
    result: GeometryResult,
    output: str | Path,
    *,
    title: str = "sldkit geometry",
    force: bool = False,
) -> Path:
    """Write a standalone HTML viewer for an already decoded geometry result.

    No files are reparsed or opened in a browser. Existing output requires force.
    Partial/empty results retain their status and render an explanatory page.
    """
    return _write(_scene(result, title), _output_path(output), force)


def view_file(
    path: str | Path,
    output: str | Path | None = None,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    force: bool = False,
) -> Path:
    """Parse a file and write an offline viewer including verified saved previews.

    Modern Parts request geometry explicitly. Other kinds show saved resources
    and decoded document information. No component references are followed.
    """
    source = Path(path)
    if not stat.S_ISREG(source.stat().st_mode):
        raise ValueError("viewer input must be a regular file")
    destination = _output_path(
        output if output is not None else source.with_suffix(".html")
    )
    if destination.resolve() == source.resolve() or (
        destination.exists() and source.exists() and destination.samefile(source)
    ):
        raise ValueError("viewer output must not overwrite the source file")
    if destination.exists() and not force:
        raise FileExistsError(f"{destination} already exists; use force to replace it")
    parsed = parse_file(source, profile=profile)
    geometry = None
    if parsed.document and parsed.document.document_kind.value is DocumentKind.PART:
        geometry = decode_geometry_file(source, profile=profile)
    scene = _scene(geometry, source.name)
    resources = _document(scene, parsed)
    preview_bytes = 0
    for name, resource in resources[:MAX_PREVIEWS]:
        if preview_bytes + resource.byte_len > MAX_PREVIEW_BYTES:
            scene["warnings"].append(f"{name}: preview byte budget exceeded; omitted.")
            continue
        extraction = extract_resource_file(source, resource, profile=profile)
        if extraction.data is None:
            scene["warnings"].append(f"{name}: saved preview could not be extracted.")
            scene["diagnostics"].extend(
                d.to_dict() for d in extraction.result.diagnostics
            )
            continue
        preview_bytes += len(extraction.data)
        try:
            media, data = _image_data(extraction.data, resource.kind)
        except ValueError as error:
            scene["warnings"].append(f"{name}: {error}; preview omitted.")
            continue
        scene["previews"].append(
            {
                "name": name,
                "url": f"data:{media};base64,{base64.b64encode(data).decode('ascii')}",
            }
        )
    if len(resources) > MAX_PREVIEWS:
        scene["warnings"].append(
            "Preview count exceeds viewer limit; remaining images omitted."
        )
    return _write(scene, destination, force)
