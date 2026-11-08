from __future__ import annotations

import json
import struct
import zlib

import pytest
import sldkit
from sldkit import _core

MODERN_MARKER = bytes.fromhex("140006000800")


def modern_frame(name: str, payload: bytes) -> bytes:
    compressor = zlib.compressobj(wbits=-zlib.MAX_WBITS)
    compressed = compressor.compress(payload) + compressor.flush()
    encoded_name = bytes(
        ((value << 4) & 0xF0) | (value >> 4) for value in name.encode("ascii")
    )
    return b"".join(
        (
            MODERN_MARKER,
            struct.pack(
                "<IIIII",
                7,
                zlib.crc32(payload),
                len(compressed),
                len(payload),
                len(encoded_name),
            ),
            encoded_name,
            compressed,
        )
    )


def document(kind: str = "PART") -> bytes:
    xml = (
        f'<root><swHeader><swFile id="0" swDocType="{kind}"/></swHeader>'
        '<swModelList><swModel id="self" swFileRef="0" '
        'swConfigurationId="0"><swConfiguration swID="0" '
        'swName="Default"/></swModel></swModelList></root>'
    )
    return (
        b"SLDK"
        + (4).to_bytes(4, "big")
        + modern_frame("swXmlContents/Features", xml.encode())
    )


def channel(width: int, kind: int, count: int, data: bytes) -> bytes:
    return struct.pack("<4I", width, kind, 2, count) + data


def table(positions: list[tuple[float, float, float]], normals: bool = True) -> bytes:
    count = len(positions)
    endpoints = 2 * count - 2
    flat = [value for point in positions for value in point]
    normal_values = [0.0, 0.0, 1.0] * count if normals else []
    return b"".join(
        (
            channel(4, 8, 1, struct.pack("<I", count)),
            channel(12, 100, count, struct.pack(f"<{len(flat)}f", *flat)),
            channel(
                12,
                100,
                len(normal_values) // 3,
                struct.pack(f"<{len(normal_values)}f", *normal_values),
            ),
            channel(4, 8, endpoints, bytes(4 * endpoints)),
            channel(4, 8, 1, struct.pack("<I", endpoints)),
            channel(1, 8, endpoints, bytes(endpoints)),
        )
    )


def class_declaration(name: bytes) -> bytes:
    return b"\xff\xff\x01\x00" + struct.pack("<H", len(name)) + name


def utf16(text: str) -> bytes:
    data = text.encode("utf-16-le")
    return b"\xff\xfe\xff" + bytes([len(data) // 2]) + data


QUAD = [(0.0, 0.0, 0.0), (0.001, 0.0, 0.0), (0.0, 0.001, 0.0), (0.001, 0.001, 0.0)]
TRIANGLE = [(0.0, 0.0, 0.0), (0.002, 0.0, 0.0), (0.0, 0.002, 0.0)]


def display_lists(*, normals: bool = True) -> bytes:
    red = b"\x33\x80" + struct.pack("<I", 0x000000FF) + bytes(12) + utf16("red")
    return b"".join(
        (
            class_declaration(b"uoTempFaceTessData_c"),
            struct.pack("<2I", 2, 1),
            table(QUAD, normals),
            red,
            b"\x01\x80",
            struct.pack("<2I", 1, 1),
            table(TRIANGLE, normals),
            utf16("moSimpleSurfIdRep_c,202,7,"),
        )
    )


def floats(data: bytes) -> list[float]:
    return list(struct.unpack(f"<{len(data) // 4}f", data))


def integers(data: bytes) -> list[int]:
    return list(struct.unpack(f"<{len(data) // 4}I", data))


def test_display_mesh_packs_tables_without_brep():
    payload = document() + modern_frame("Contents/DisplayLists", display_lists())
    result = sldkit.decode_display_mesh_bytes(payload, filename="cache.SLDPRT")
    native, *_ = _core.decode_display_mesh_bytes(payload, "cache.SLDPRT", "desktop")
    assert result.to_dict() == json.loads(native)
    assert result.status is sldkit.DisplayMeshStatus.DECODED
    mesh = result.mesh
    assert mesh is not None
    assert mesh.document_kind is sldkit.DocumentKind.PART
    assert mesh.length_unit == "millimeter"
    assert (mesh.vertex_count, mesh.triangle_count) == (7, 3)
    assert [part.triangle_count for part in mesh.parts] == [2, 1]
    assert floats(result.positions)[:6] == pytest.approx([0, 0, 0, 1, 0, 0])
    assert integers(result.indices) == [0, 1, 2, 1, 3, 2, 4, 5, 6]
    assert len(integers(result.edges)) == 2 * mesh.edge_count == 2 * (4 + 3)
    assert mesh.bounds is not None
    assert mesh.bounds.max == pytest.approx((2.0, 2.0, 0.0))
    first, second = mesh.parts
    assert first.color == pytest.approx((1.0, 0.0, 0.0, 1.0))
    assert first.color_source is sldkit.DisplayColorSource.FACE
    assert first.appearance_name == "red"
    assert second.surface_references[0].feature_source_id == 202
    assert (second.vertex_start, second.triangle_start) == (4, 2)


def test_display_mesh_matches_geometry_tessellation():
    payload = document() + modern_frame("Contents/DisplayLists", display_lists())
    display = sldkit.decode_display_mesh_bytes(payload, filename="cache.SLDPRT")
    geometry = sldkit.decode_geometry_bytes(payload, filename="cache.SLDPRT")
    assert geometry.geometry is not None
    meshes = geometry.geometry.model.tessellations
    assert display.mesh is not None
    assert [part.vertex_count for part in display.mesh.parts] == [
        len(mesh.vertices) for mesh in meshes
    ]
    positions = floats(display.positions)
    expected = [
        value for mesh in meshes for vertex in mesh.vertices for value in vertex
    ]
    assert positions == pytest.approx(expected, abs=1e-6)


def test_display_mesh_derives_missing_normals():
    payload = document() + modern_frame(
        "Contents/DisplayLists", display_lists(normals=False)
    )
    result = sldkit.decode_display_mesh_bytes(payload, filename="cache.SLDPRT")
    assert result.mesh is not None
    assert all(part.normals_derived for part in result.mesh.parts)
    assert len(result.normals) == len(result.positions)
    assert "display.normals_derived" in {item.code for item in result.diagnostics}


def test_display_mesh_without_display_data_is_unsupported():
    result = sldkit.decode_display_mesh_bytes(document(), filename="empty.SLDPRT")
    assert result.status is sldkit.DisplayMeshStatus.UNSUPPORTED
    assert "display.no_display_data" in {item.code for item in result.diagnostics}
    assert result.positions == b""
    with pytest.raises(sldkit.DisplayMeshError) as caught:
        sldkit.decode_display_mesh_bytes(
            document(), filename="empty.SLDPRT", strict=True
        )
    assert caught.value.result == result


def test_unreadable_display_stream_is_reported():
    payload = document() + modern_frame("Contents/DisplayLists", b"not a table" * 8)
    result = sldkit.decode_display_mesh_bytes(payload, filename="bad.SLDPRT")
    assert result.status is sldkit.DisplayMeshStatus.UNSUPPORTED
    codes = {item.code for item in result.diagnostics}
    assert "display.stream_unreadable" in codes
    assert result.mesh is not None
    assert result.mesh.streams[0].part_count == 0


def test_assembly_display_tables_are_returned_with_placement_note():
    payload = document("ASSEMBLY") + modern_frame(
        "Contents/DisplayLists", display_lists()
    )
    result = sldkit.decode_display_mesh_bytes(payload, filename="asm.SLDASM")
    assert result.mesh is not None
    assert result.mesh.document_kind is sldkit.DocumentKind.ASSEMBLY
    codes = {item.code for item in result.diagnostics}
    assert "display.assembly_placement_unverified" in codes


PLATE_ID = bytes.fromhex("8fea5b1444e29701")
PLATE_REPEAT_ID = bytes.fromhex("0519d1368b6f0417")
BOLT_ID = bytes.fromhex("3abcdc0f752f7b0a")
NO_SECTION = 0xFFFFFFFF


def directory_record(
    path: str, reference: bytes, ident: bytes, stream: str, section: int, bodies: int
) -> bytes:
    out = utf16(path) + struct.pack("<I", 1234) + bytes(8) + reference + ident
    out += utf16(stream) + struct.pack("<II", section, bodies)
    for body in range(bodies):
        out += struct.pack("<III", 101 + body, 80, 6)
    return out


def face_directory() -> bytes:
    """Owner, repeat and nested owner; the sub-assembly node takes tree index 3."""
    records = [
        (1, directory_record("Plate-1@Asm", bytes(8), PLATE_ID, "000-000-001", 0, 1)),
        (2, directory_record("Plate-2@Asm", PLATE_ID, PLATE_REPEAT_ID, "", NO_SECTION, 1)),
        (4, directory_record("Sub-1@Asm/Bolt-1@Sub", bytes(8), BOLT_ID, "000-000-001", 1, 1)),
    ]
    out = struct.pack("<4I", 2, 1, len(records), records[0][0])
    for index, (tree_index, record) in enumerate(records):
        if index:
            out += struct.pack("<I", tree_index)
        out += record
    return out


def face_trailer(reference: str) -> bytes:
    return bytes(8) + struct.pack("<I", 1) + bytes(80) + utf16(reference)


def face_stream() -> bytes:
    return b"".join(
        (
            struct.pack("<I", 0),
            PLATE_ID,
            struct.pack("<I", 2),
            struct.pack("<2I", 2, 1),
            table(QUAD),
            face_trailer("moSimpleSurfIdRep_c,7,1, "),
            struct.pack("<2I", 1, 1),
            table(TRIANGLE, normals=False),
            face_trailer("moEndFace3IntSurfIdRep_c,9,0,0,0, "),
            struct.pack("<I", 99),
            struct.pack("<I", 1),
            BOLT_ID,
            struct.pack("<I", 1),
            struct.pack("<2I", 1, 1),
            table(TRIANGLE),
            face_trailer("moSimpleSurfIdRep_c,3,2, "),
            struct.pack("<I", 5),
        )
    )


def test_face_tessellations_assembly_lists_instances():
    payload = (
        document("ASSEMBLY")
        + modern_frame("Contents/DisplayLists", b"environment only " * 4)
        + modern_frame("FaceTessellations/Directory", face_directory())
        + modern_frame("FaceTessellations/000-000-001", face_stream())
    )
    result = sldkit.decode_display_mesh_bytes(payload, filename="asm.SLDASM")
    native, *_ = _core.decode_display_mesh_bytes(payload, "asm.SLDASM", "desktop")
    assert result.to_dict() == json.loads(native)
    assert result.status is sldkit.DisplayMeshStatus.DECODED
    mesh = result.mesh
    assert mesh is not None
    assert [stream.stream_path for stream in mesh.streams] == [
        "Contents/DisplayLists",
        "FaceTessellations/000-000-001",
    ]
    assert [stream.part_count for stream in mesh.streams] == [0, 3]
    assert mesh.triangle_count == 4
    assert [(part.stream_index, part.instance_index) for part in mesh.parts] == [
        (1, 0),
        (1, 0),
        (1, 2),
    ]
    assert mesh.parts[1].normals_derived
    plate, repeat, bolt = mesh.instances
    assert isinstance(plate, sldkit.DisplayMeshInstance)
    assert (plate.name, plate.number, plate.tables_from) == ("Plate", 1, 0)
    assert (plate.stream_index, plate.section_index) == (1, 0)
    assert (plate.declared_face_count, plate.face_count, plate.body_count) == (2, 2, 1)
    assert (repeat.path, repeat.tables_from, repeat.stream_index) == (
        "Plate-2@Asm",
        0,
        None,
    )
    assert (bolt.path, bolt.tree_index, bolt.tables_from) == (
        "Sub-1@Asm/Bolt-1@Sub",
        4,
        2,
    )
    codes = {item.code for item in result.diagnostics}
    assert "display.face_tessellations_directory" in codes
    assert "display.assembly_placement_unverified" in codes
    assert "display.stream_unreadable" not in codes
    assert floats(result.positions)[:6] == pytest.approx([0, 0, 0, 1, 0, 0])


def test_drawing_documents_are_unsupported():
    payload = document("DRAWING") + modern_frame(
        "Contents/DisplayLists", display_lists()
    )
    result = sldkit.decode_display_mesh_bytes(payload, filename="sheet.SLDDRW")
    assert result.status is sldkit.DisplayMeshStatus.UNSUPPORTED
    assert result.mesh is None


def test_display_mesh_file_entry_point(tmp_path):
    path = tmp_path / "part.SLDPRT"
    path.write_bytes(
        document() + modern_frame("Contents/DisplayLists", display_lists())
    )
    result = sldkit.decode_display_mesh_file(path, profile="service")
    assert result.status is sldkit.DisplayMeshStatus.DECODED
    assert result.mesh is not None
    assert result.mesh.source.input_kind is sldkit.SourceInputKind.PATH


def test_display_cli_prints_json_without_buffers(tmp_path, capsys):
    from sldkit.cli import main

    path = tmp_path / "part.SLDPRT"
    path.write_bytes(
        document() + modern_frame("Contents/DisplayLists", display_lists())
    )
    assert main(["display", str(path), "--limits", "service"]) == 0
    output = json.loads(capsys.readouterr().out)
    assert output["status"] == "decoded"
    assert output["mesh"]["triangle_count"] == 3
    assert "positions" not in output

    empty = tmp_path / "empty.SLDPRT"
    empty.write_bytes(document())
    assert main(["display", str(empty)]) == 1


def _oracle():
    import runpy
    from pathlib import Path

    return runpy.run_path(
        str(Path(__file__).parents[1] / "scripts/validate_display_oracle.py")
    )


def test_display_oracle_grid_and_verdicts():
    oracle = _oracle()
    grid = oracle["SegmentGrid"]([((0.0, 0.0, 0.0), (10.0, 0.0, 0.0))], 0.5)
    assert grid.nearest((5.0, 0.3, 0.0)) == pytest.approx(0.3)
    assert grid.nearest((5.0, 9.0, 0.0)) == float("inf")
    verdict = oracle["verdict"]
    assert verdict({"edge_on_display": 1.0, "outline_covered": 0.99}, 0.98) == "match"
    assert (
        verdict({"edge_on_display": 1.0, "outline_covered": 0.2}, 0.98)
        == "brep_incomplete"
    )
    assert (
        verdict({"edge_on_display": 0.1, "outline_covered": 0.9}, 0.98)
        == "brep_off_display"
    )
