from __future__ import annotations

import base64
import json
import re
import struct
from dataclasses import replace

import pytest
import sldkit
from sldkit.cli import main
from sldkit.viewer import _html, view_file, write_html
from viewer_fixtures import geometry_result, png, rle_dib, source_bytes


def scene(path):
    match = re.search(
        r'<script id="sldkit-scene" type="application/json">(.*?)</script>',
        path.read_text(encoding="utf-8"),
        re.S,
    )
    assert match
    return json.loads(match[1])


def test_html_retains_mesh_coordinates_indices_units_and_partial_status(tmp_path):
    result = geometry_result()
    before = result.to_dict()
    output = write_html(result, tmp_path / "part.html")
    data = scene(output)
    assert data["geometry_status"] == "partial"
    assert data["unit"] == "millimeter"
    assert data["counts"] == {"meshes": 2, "vertices": 16, "triangles": 24}
    assert (
        data["meshes"][0]["vertices"]
        == before["geometry"]["model"]["tessellations"][0]["vertices"]
    )
    assert (
        data["meshes"][0]["triangles"]
        == before["geometry"]["model"]["tessellations"][0]["triangles"]
    )
    assert result.to_dict() == before
    assert data["configurations"][1]["body_ids"] == []
    assert data["configurations"][2]["body_ids"] is None
    assert "Three.js 0.180.0" in output.read_text()
    assert not re.search(r"<script[^>]+src=", output.read_text())


def test_source_strings_cannot_terminate_embedded_json_or_replace_template(tmp_path):
    title = '</script><img src=x onerror="alert(1)">@@SCRIPT@@ & 日本語'
    output = write_html(geometry_result(), tmp_path / "escape.html", title=title)
    content = output.read_text()
    assert title not in content
    assert scene(output)["title"] == title
    assert content.count("</script>") == 2
    assert "&lt;/script&gt;&lt;img" in content


@pytest.mark.parametrize("damage", ["nan", "index", "negative", "float_index"])
def test_invalid_mesh_is_omitted_without_dropping_valid_mesh(tmp_path, damage):
    result = geometry_result()
    g = result.geometry
    mesh = g.model.tessellations[0]
    if damage == "nan":
        mesh = replace(mesh, vertices=((float("nan"), 0, 0), *mesh.vertices[1:]))
    else:
        index = {"index": 100, "negative": -1, "float_index": 1.5}[damage]
        mesh = replace(mesh, triangles=((0, 1, index),))
    result = replace(
        result,
        geometry=replace(
            g,
            model=replace(
                g.model,
                tessellations=(mesh, g.model.tessellations[1]),
            ),
        ),
    )
    data = scene(write_html(result, tmp_path / "invalid.html"))
    assert data["counts"]["meshes"] == 1
    assert data["meshes"][0]["id"] == "mesh-1"
    assert "invalid coordinates or triangle indices" in data["warnings"][0]
    assert data["geometry_status"] == "partial"


def test_invalid_optional_channels_do_not_remove_mesh(tmp_path):
    result = geometry_result()
    g = result.geometry
    mesh = replace(
        g.model.tessellations[0],
        normals=((0, 1, 0),),
        corner_normals=((0, 1, 0),),
        feature_edges=((0, 99),),
    )
    result = replace(
        result, geometry=replace(g, model=replace(g.model, tessellations=(mesh,)))
    )
    data = scene(write_html(result, tmp_path / "optional.html"))
    assert data["counts"]["meshes"] == 1
    assert len(data["warnings"]) == 3
    assert data["meshes"][0]["normals"] == []
    assert data["meshes"][0]["corner_normals"] == []
    assert data["meshes"][0]["feature_edges"] == []


def test_display_budgets_are_explicit_and_do_not_modify_parser_status(
    tmp_path, monkeypatch
):
    monkeypatch.setattr(_html, "MAX_VERTICES", 8)
    data = scene(write_html(geometry_result(), tmp_path / "limited.html"))
    assert data["counts"]["meshes"] == 1
    assert "budget exceeded" in data["warnings"][0]
    assert data["geometry_status"] == "partial"
    monkeypatch.setattr(_html, "MAX_JSON_BYTES", 1)
    output = tmp_path / "too-large.html"
    with pytest.raises(ValueError, match="JSON exceeds"):
        write_html(geometry_result(), output)
    assert not output.exists()


def test_view_file_embeds_verified_png_and_drawing_information(tmp_path):
    source = tmp_path / "drawing.SLDDRW"
    source.write_bytes(source_bytes(drawing=True))
    data = scene(view_file(source))
    assert data["geometry_status"] == "not_requested"
    assert data["document"]["kind"] == "drawing"
    assert data["document"]["sheets"][0]["name"] == "Sheet One"
    assert data["previews"][0]["url"].startswith("data:image/png;base64,")
    assert base64.b64decode(data["previews"][0]["url"].split(",")[1]) == png()


def test_preview_extraction_failure_is_reported(tmp_path, monkeypatch):
    source = tmp_path / "part.SLDPRT"
    source.write_bytes(source_bytes())
    real = _html.extract_resource_file

    def without_payload(*args, **kwargs):
        return replace(real(*args, **kwargs), data=None)

    monkeypatch.setattr(_html, "extract_resource_file", without_payload)
    data = scene(view_file(source))
    assert data["previews"] == []
    assert "could not be extracted" in data["warnings"][0]


def test_unresolved_and_unsupported_results_remain_explicit(tmp_path):
    data = scene(
        write_html(geometry_result(unresolved=True), tmp_path / "unresolved.html")
    )
    assert data["meshes"][0]["body_id"] is None
    result = sldkit.decode_geometry_bytes(b"unknown input")
    data = scene(write_html(result, tmp_path / "unsupported.html"))
    assert data["geometry_status"] == "unsupported"
    assert data["meshes"] == []
    assert data["diagnostics"]


def test_output_requires_explicit_overwrite_and_never_replaces_source(tmp_path):
    source = tmp_path / "part.SLDPRT"
    original = source_bytes()
    source.write_bytes(original)
    output = view_file(source)
    with pytest.raises(FileExistsError):
        view_file(source)
    assert view_file(source, force=True) == output
    alias = tmp_path / "alias.html"
    alias.hardlink_to(source)
    with pytest.raises(ValueError, match="source file"):
        view_file(source, alias, force=True)
    with pytest.raises(ValueError, match=".html"):
        view_file(source, source, force=True)
    assert source.read_bytes() == original


def test_cli_writes_html_and_reports_output_errors_without_traceback(tmp_path, capsys):
    source = tmp_path / "part.SLDPRT"
    source.write_bytes(source_bytes())
    output = tmp_path / "part.html"
    assert main(["view", str(source), "-o", str(output)]) == 0
    assert capsys.readouterr().out.strip() == str(output)
    assert scene(output)["previews"]
    assert main(["view", str(source), "-o", str(output)]) == 2
    assert "already exists" in capsys.readouterr().err


def test_cli_rejects_missing_input_and_directories_without_writing(tmp_path, capsys):
    output = tmp_path / "missing.html"
    assert main(["view", str(tmp_path / "missing.SLDPRT"), "-o", str(output)]) == 2
    assert "sldkit view:" in capsys.readouterr().err
    assert not output.exists()
    assert main(["view", str(tmp_path), "-o", str(output)]) == 2
    assert "regular file" in capsys.readouterr().err
    assert not output.exists()


@pytest.mark.parametrize(
    "header,depth,compression,colors",
    [
        (40, 24, 0, 0),
        (40, 8, 0, 2),
        (40, 16, 3, 0),
        (108, 32, 3, 0),
        (124, 24, 0, 0),
    ],
)
def test_dib_file_header_preserves_pixels_and_palette(
    header, depth, compression, colors
):
    dib = bytearray(header)
    struct.pack_into("<IiiHHI", dib, 0, header, 1, 1, 1, depth, compression)
    struct.pack_into("<I", dib, 32, colors)
    dib += b"\0" * (colors * 4 + (12 if header == 40 and compression == 3 else 0))
    offset = len(dib) + 14
    dib += b"\x20\x40\x60\0"
    media, bmp = _html._image_data(bytes(dib), sldkit.BinaryResourceKind.PREVIEW_DIB)
    assert media == "image/bmp"
    assert struct.unpack_from("<2sIHHI", bmp) == (b"BM", len(bmp), 0, 0, offset)
    assert bmp[14:] == dib


def test_malformed_or_oversized_images_are_rejected():
    with pytest.raises(ValueError, match="PNG"):
        _html._image_data(b"<svg/>", sldkit.BinaryResourceKind.PREVIEW_PNG)
    image = bytearray(png())
    struct.pack_into(">II", image, 16, 1_000_000, 1_000_000)
    with pytest.raises(ValueError, match="dimensions"):
        _html._image_data(bytes(image), sldkit.BinaryResourceKind.PREVIEW_PNG)
    with pytest.raises(ValueError, match="DIB"):
        _html._image_data(b"\0" * 40, sldkit.BinaryResourceKind.PREVIEW_DIB)


def test_rle_dib_uses_compressed_size_and_rejects_truncation():
    dib = rle_dib()
    media, bmp = _html._image_data(dib, sldkit.BinaryResourceKind.PREVIEW_DIB)
    assert media == "image/bmp"
    assert bmp[14:] == dib
    with pytest.raises(ValueError, match="truncated"):
        _html._image_data(dib[:-1], sldkit.BinaryResourceKind.PREVIEW_DIB)
    damaged = bytearray(dib)
    struct.pack_into("<I", damaged, 20, 0)
    with pytest.raises(ValueError, match="declared image size"):
        _html._image_data(bytes(damaged), sldkit.BinaryResourceKind.PREVIEW_DIB)
