"""Source-less fixtures shared by Python and offline browser viewer checks."""

from __future__ import annotations

import base64
import struct
import sys
import zlib
from dataclasses import replace
from pathlib import Path

import sldkit
from sldkit.viewer import _html, view_file, write_html


def modern_stream(name: str, payload: bytes) -> bytes:
    compressor = zlib.compressobj(wbits=-zlib.MAX_WBITS)
    compressed = compressor.compress(payload) + compressor.flush()
    encoded = bytes(((v << 4) & 0xF0) | (v >> 4) for v in name.encode("ascii"))
    return (
        bytes.fromhex("140006000800")
        + struct.pack(
            "<IIIII",
            7,
            zlib.crc32(payload),
            len(compressed),
            len(payload),
            len(encoded),
        )
        + encoded
        + compressed
    )


def png() -> bytes:
    def chunk(kind: bytes, value: bytes) -> bytes:
        body = kind + value
        return (
            struct.pack(">I", len(value)) + body + struct.pack(">I", zlib.crc32(body))
        )

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(b"\x00\x10\x80\x60\xff"))
        + chunk(b"IEND", b"")
    )


def source_bytes(*, drawing: bool = False) -> bytes:
    kind = "DRAWING" if drawing else "PART"
    data = b"SLDK\x00\x00\x00\x04" + modern_stream(
        "swXmlContents/Features",
        f'<root><swHeader><swFile swDocType="{kind}"/></swHeader></root>'.encode(),
    )
    data += modern_stream("PreviewPNG", png())
    if drawing:
        data += modern_stream(
            "swXmlContents/KeyWords",
            b'<Keywords><Sheet Type="Sheet" id="s1" Name="Sheet One">'
            b'<View id="v1">part.SLDPRT</View></Sheet></Keywords>',
        )
    return data


def rle_dib() -> bytes:
    header = bytearray(40)
    struct.pack_into("<IiiHHII", header, 0, 40, 2, 1, 1, 8, 1, 6)
    struct.pack_into("<I", header, 32, 2)
    return bytes(header) + b"\0\0\0\0\x60\x80\x10\0" + b"\x02\x01\0\0\0\x01"


def geometry_result(*, unresolved: bool = False) -> sldkit.GeometryResult:
    result = sldkit.decode_geometry_bytes(source_bytes(), filename="fixture.SLDPRT")
    assert result.geometry is not None
    provenance = {
        "stream": None,
        "offset": None,
        "tag": "fixture",
        "exactness": "derived",
    }
    meshes = []
    bodies = []
    corners = (
        (-1, -1, -1),
        (1, -1, -1),
        (1, 1, -1),
        (-1, 1, -1),
        (-1, -1, 1),
        (1, -1, 1),
        (1, 1, 1),
        (-1, 1, 1),
    )
    triangles = (
        (0, 2, 1),
        (0, 3, 2),
        (4, 5, 6),
        (4, 6, 7),
        (0, 1, 5),
        (0, 5, 4),
        (3, 7, 6),
        (3, 6, 2),
        (0, 4, 7),
        (0, 7, 3),
        (1, 2, 6),
        (1, 6, 5),
    )
    for i in range(2):
        body_id = f"body-{i}"
        bodies.append(
            sldkit.GeometryBody.from_dict(
                {
                    "id": body_id,
                    "kind": "solid",
                    "provenance": provenance,
                }
            )
        )
        meshes.append(
            sldkit.GeometryTessellation.from_dict(
                {
                    "id": f"mesh-{i}",
                    "body_id": None if unresolved else body_id,
                    "vertices": [[x + 3 * i, y, z] for x, y, z in corners],
                    "triangles": triangles,
                    "feature_edges": [[0, 1], [1, 2]],
                    "provenance": provenance,
                }
            )
        )
    bodies.append(replace(bodies[0], id="body-without-cache"))
    model = replace(
        result.geometry.model, bodies=tuple(bodies), tessellations=tuple(meshes)
    )
    configs = tuple(
        sldkit.GeometryConfigurationState.from_dict(
            {
                "id": name,
                "ordinal": i,
                "name": name,
                "body_ids": ids,
            }
        )
        for i, (name, ids) in enumerate(
            [
                ("First", ["body-0"]),
                ("Empty", []),
                ("Unknown", None),
                ("No cache", ["body-without-cache"]),
            ]
        )
    )
    return replace(
        result, geometry=replace(result.geometry, model=model, configurations=configs)
    )


def generate(directory: Path) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    write_html(
        geometry_result(), directory / "meshes.html", title="Two source-less cubes"
    )
    write_html(geometry_result(unresolved=True), directory / "unresolved.html")
    write_html(
        geometry_result(),
        directory / "escaped.html",
        title='</script><script>globalThis.injected=true</script>@@SCRIPT@@<img src="https://invalid.test/x">',
    )
    for name, drawing in [("preview", False), ("drawing", True)]:
        path = directory / (name + (".SLDDRW" if drawing else ".SLDPRT"))
        path.write_bytes(source_bytes(drawing=drawing))
        view_file(path)
    write_html(sldkit.decode_geometry_bytes(b"unknown"), directory / "empty.html")
    data = _html._scene(None, "Source-less RLE preview")
    media, bmp = _html._image_data(rle_dib(), sldkit.BinaryResourceKind.PREVIEW_DIB)
    data["previews"] = [
        {
            "name": "RLE8",
            "url": f"data:{media};base64,{base64.b64encode(bmp).decode()}",
        }
    ]
    _html._write(data, directory / "rle.html", False)


if __name__ == "__main__":
    generate(Path(sys.argv[1]))
