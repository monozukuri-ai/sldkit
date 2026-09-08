from __future__ import annotations

import hashlib
import io
import json
import struct
import zipfile
import zlib

import pytest
import sldkit
from sldkit import _core

OLE2_SIGNATURE = bytes.fromhex("d0cf11e0a1b11ae1")
MODERN_MARKER = bytes.fromhex("140006000800")


def modern_file(payload: bytes = b"payload", name: str = "Contents/Test") -> bytes:
    return modern_streams(((name, payload),))


def modern_streams(streams: tuple[tuple[str, bytes], ...]) -> bytes:
    output = bytearray(b"SLDK" + (4).to_bytes(4, "big"))
    for name, payload in streams:
        output.extend(modern_frame(name, payload))
    return bytes(output)


def modern_frame(name: str, payload: bytes) -> bytes:
    compressor = zlib.compressobj(wbits=-zlib.MAX_WBITS)
    compressed = compressor.compress(payload) + compressor.flush()
    encoded_name = bytes(
        ((value << 4) & 0xF0) | (value >> 4) for value in name.encode("ascii")
    )
    frame = b"".join(
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
    return frame


def assembly_file(*components: tuple[str, str, str, bool]) -> bytes:
    files = "".join(
        f'<swFile id="{index}" swDocType="{kind}" swPath="{path}"/>'
        for index, (path, kind, _name, _suppressed) in enumerate(components, 1)
    )
    references = "".join(
        f'<swReference swModelRef="m{index}" swName="{name}" '
        f'swSuppressed="{"YES" if suppressed else "NO"}"/>'
        for index, (_path, _kind, name, suppressed) in enumerate(components, 1)
    )
    models = "".join(
        f'<swModel id="m{index}" swFileRef="{index}" swConfigurationName="Default"/>'
        for index, _component in enumerate(components, 1)
    )
    xml = (
        '<root><swHeader><swFile id="0" swDocType="ASSEMBLY"/>'
        f"{files}</swHeader><swModelList>"
        '<swModel id="self" swFileRef="0" swConfigurationId="0">'
        '<swConfiguration swID="0" swName="Default"/>'
        f"{references}</swModel>{models}</swModelList></root>"
    )
    return modern_file(xml.encode(), "swXmlContents/COMPINSTANCETREE")


def part_file() -> bytes:
    xml = (
        '<root><swHeader><swFile id="0" swDocType="PART"/></swHeader>'
        '<swModelList><swModel id="self" swFileRef="0" '
        'swConfigurationId="0"><swConfiguration swID="0" '
        'swName="Default"/></swModel></swModelList></root>'
    )
    return modern_file(xml.encode(), "swXmlContents/Features")


def drawing_keywords() -> bytes:
    return (
        b'<Keywords><Note id="n1">preserve</Note>'
        b'<Sheet Type="Sheet" id="s1" Name="Sheet1">'
        b'<PaperSize Width="1"/><View id="v1" Name="Front" '
        b'Description="Machined">child.SLDPRT</View>'
        b'<View id="v1" Name="Right">other.SLDPRT</View></Sheet>'
        b'<Sheet Type="Sheet Format" id="sf1"/><Sketch id="sk1"/>'
        b'<View id="global">detached.SLDPRT</View></Keywords>'
    )


def drawing_file() -> bytes:
    return modern_streams(
        (
            ("swXmlContents/KeyWords", drawing_keywords()),
            ("Contents/Definition", b"def"),
            ("Contents/DisplayLists", b"display"),
            ("Contents/VBLists", b"vb"),
        )
    )


def stored_zip() -> bytes:
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_STORED) as archive:
        archive.writestr("docProps/test.xml", b"zip-payload")
    return output.getvalue()


@pytest.mark.parametrize(
    ("payload", "envelope", "confidence"),
    [
        (OLE2_SIGNATURE, sldkit.Envelope.OLE2_CFB, sldkit.ProbeConfidence.HIGH),
        (b"PK\x03\x04", sldkit.Envelope.ZIP_OPC, sldkit.ProbeConfidence.HIGH),
        (
            b"header" + MODERN_MARKER,
            sldkit.Envelope.MODERN_CHUNK,
            sldkit.ProbeConfidence.MEDIUM,
        ),
    ],
)
def test_probe_bytes_uses_content(payload, envelope, confidence):
    result = sldkit.probe_bytes(payload)

    assert result.status is sldkit.ProbeStatus.RECOGNIZED
    assert result.envelope is envelope
    assert result.confidence is confidence
    assert result.coverage.total_bytes == len(payload)
    assert result.coverage.decoded_bytes == 0


def test_parse_retains_identity_and_reports_partial_semantic_profile():
    payload = modern_file()
    result = sldkit.parse_bytes(payload, filename="widget.SLDPRT")

    assert result.status is sldkit.ParseStatus.PARTIAL
    assert result.document is not None
    assert result.document.envelope.value is sldkit.Envelope.MODERN_CHUNK
    assert result.document.document_kind.value is sldkit.DocumentKind.PART
    assert result.document.document_kind.origin is sldkit.ValueOrigin.HINT
    assert result.document.source.sha256 == hashlib.sha256(payload).hexdigest()
    assert len(result.document.configurations) == 1
    assert result.document.configurations[0].index.value == 0
    assert result.document.configurations[0].index.origin is sldkit.ValueOrigin.INFERRED
    assert result.inventory is not None
    assert len(result.inventory.entries) == 1
    assert result.semantic_coverage is not None
    assert result.semantic_coverage.uninterpreted_streams == 1
    assert {item.code for item in result.diagnostics} == {
        "modern.configuration_index_inferred",
        "format.modern_profile_partial",
    }


def test_malformed_and_unsupported_are_observably_distinct():
    malformed = sldkit.parse_bytes(b"")
    unsupported = sldkit.parse_bytes(b"not a known container")

    assert malformed.status is sldkit.ParseStatus.MALFORMED
    assert malformed.diagnostics[0].kind is sldkit.DiagnosticKind.MALFORMED
    assert unsupported.status is sldkit.ParseStatus.UNSUPPORTED
    assert unsupported.diagnostics[0].kind is sldkit.DiagnosticKind.UNSUPPORTED


def test_strict_mode_retains_result_on_exception():
    with pytest.raises(sldkit.ParseError) as caught:
        sldkit.parse_bytes(modern_file(), strict=True)

    assert caught.value.result.status is sldkit.ParseStatus.PARTIAL


def test_geometry_api_is_explicit_typed_and_matches_native_json():
    payload = part_file()

    result = sldkit.decode_geometry_bytes(payload, filename="fixture.SLDPRT")
    native = json.loads(
        _core.decode_geometry_bytes_json(payload, "fixture.SLDPRT", "desktop")
    )

    assert result.status is sldkit.GeometryStatus.PARTIAL
    assert result.geometry is not None
    assert result.geometry.source.input_kind is sldkit.SourceInputKind.BYTES
    assert result.geometry.source.sha256 == hashlib.sha256(payload).hexdigest()
    assert result.geometry.fidelity.geometry_transferred is False
    assert result.geometry.model.bodies == ()
    assert result.geometry.model.constructions == ()
    assert (
        result.geometry.fidelity.byte_coverage.partition_status
        is sldkit.GeometryBytePartitionStatus.INCOMPLETE
    )
    assert result.geometry.fidelity.byte_coverage.classified_active_bytes == 0
    assert result.geometry.fidelity.byte_coverage.partition_domain_bytes == 0
    assert result.geometry.fidelity.byte_coverage.typed_bytes is None
    assert result.geometry.fidelity.byte_coverage.uninterpreted_bytes is None
    assert sldkit.GeometryConstructionDomain.SURFACE.value == "surface"
    assert sldkit.GeometryByteStorage.WRAPPED_ZLIB.value == "wrapped_zlib"
    assert sldkit.GeometryByteOffsetBasis.PARASOLID_BODY.value == "parasolid_body"
    assert sldkit.GeometryPcurveState.__name__ == "GeometryPcurveState"
    assert (
        sldkit.GeometryTessellationTriangleGroup.__name__
        == "GeometryTessellationTriangleGroup"
    )
    assert result.to_dict() == native
    assert {item.code for item in result.diagnostics} == {
        "geometry.not_transferred",
        "geometry.byte_partition_incomplete",
    }


def test_geometry_strict_mode_retains_partial_result():
    with pytest.raises(sldkit.GeometryError) as caught:
        sldkit.decode_geometry_bytes(
            part_file(), filename="fixture.SLDPRT", strict=True
        )

    assert caught.value.result.status is sldkit.GeometryStatus.PARTIAL


def display_list_payload() -> bytes:
    """One source-less display triangle, independent of a Parasolid body."""

    def channel(width: int, kind: int, count: int, data: bytes) -> bytes:
        return struct.pack("<4I", width, kind, 2, count) + data

    return b"".join(
        (
            b"uoTempFaceTessData_c",
            struct.pack("<2I", 1, 1),
            channel(4, 8, 1, struct.pack("<I", 3)),
            channel(12, 100, 3, struct.pack("<9f", 0, 0, 0, 1, 0, 0, 0, 1, 0)),
            channel(12, 100, 3, struct.pack("<9f", 0, 0, 1, 0, 0, 1, 0, 0, 1)),
            channel(4, 8, 4, bytes(16)),
            channel(4, 8, 1, struct.pack("<I", 4)),
            channel(1, 8, 4, bytes(4)),
        )
    )


@pytest.mark.parametrize("truncated", [False, True])
def test_geometry_display_cache_without_brep(truncated):
    display = display_list_payload()
    if truncated:
        display = display[: len(display) // 2]
    payload = part_file() + modern_frame("Contents/DisplayLists", display)
    result = sldkit.decode_geometry_bytes(payload, filename="cache.SLDPRT")
    native = json.loads(
        _core.decode_geometry_bytes_json(payload, "cache.SLDPRT", "desktop")
    )
    assert result.to_dict() == native
    assert sldkit.GeometryResult.from_dict(native) == result
    assert result.status is sldkit.GeometryStatus.PARTIAL
    assert result.geometry is not None
    assert result.geometry.fidelity.geometry_transferred is False
    assert result.geometry.model.bodies == ()
    assert result.geometry.model.faces == ()
    meshes = result.geometry.model.tessellations
    assert len(meshes) == (0 if truncated else 1)
    codes = {d.code for d in result.diagnostics}
    assert "geometry.not_transferred" in codes
    assert ("geometry.display_cache_transferred" in codes) is not truncated
    if not truncated:
        mesh = meshes[0]
        assert mesh.vertices == ((0, 0, 0), (1000, 0, 0), (0, 1000, 0))
        assert mesh.triangles == ((0, 1, 2),)
        assert mesh.normals == ((0, 0, 1),) * 3
        assert mesh.body_id is None
        assert mesh.face_ids == ()
        assert mesh.provenance.stream == "Contents/DisplayLists"
        assert len(mesh.channels) == 6
        with pytest.raises(sldkit.GeometryError) as caught:
            sldkit.decode_geometry_bytes(payload, filename="cache.SLDPRT", strict=True)
        assert caught.value.result.geometry.model.tessellations == meshes


def test_geometry_rejects_non_part_document_kind():
    result = sldkit.decode_geometry_bytes(assembly_file(), filename="fixture.SLDASM")

    assert result.status is sldkit.GeometryStatus.UNSUPPORTED
    assert result.geometry is None
    assert result.diagnostics[-1].code == "geometry.document_kind_unsupported"


def test_drawing_structure_api_is_typed_deterministic_and_matches_native_json():
    payload = drawing_file()

    result = sldkit.decode_drawing_structure_bytes(
        payload, filename="fixture.SLDDRW"
    )
    repeated = sldkit.decode_drawing_structure_bytes(
        payload, filename="fixture.SLDDRW"
    )
    native = json.loads(
        _core.decode_drawing_structure_bytes_json(
            payload, "fixture.SLDDRW", "desktop"
        )
    )

    assert result == repeated
    assert result.status is sldkit.DrawingStructureStatus.PARTIAL
    assert result.structure is not None
    assert result.structure.source.input_kind is sldkit.SourceInputKind.BYTES
    assert result.structure.source.sha256 == hashlib.sha256(payload).hexdigest()
    assert result.structure.coverage.record_count == 9
    assert result.structure.coverage.sheet_record_count == 2
    assert result.structure.coverage.supported_sheet_count == 1
    assert result.structure.coverage.sheet_view_count == 2
    assert result.structure.coverage.unassigned_view_record_count == 1
    assert result.structure.coverage.candidate_stream_count == 3
    assert result.structure.coverage.candidate_stream_bytes == 12
    assert (
        result.structure.coverage.partition_status
        is sldkit.DrawingBytePartitionStatus.INCOMPLETE
    )
    assert result.structure.coverage.typed_bytes is None
    assert result.structure.coverage.uninterpreted_bytes is None
    assert len(result.structure.sheets) == 1
    assert len(result.structure.views) == 2
    assert (
        result.structure.views[0].sheet_record_id
        == result.structure.sheets[0].record_id
    )
    assert result.structure.views[0].source_id == result.structure.views[1].source_id
    assert result.structure.views[0].record_id != result.structure.views[1].record_id
    assert all(
        not carrier.record_framing_verified
        for carrier in result.structure.source_streams
    )
    view = next(
        record
        for record in result.structure.records
        if record.record_class is sldkit.DrawingRecordClass.VIEW
        and record.direct_text == "child.SLDPRT"
    )
    raw = drawing_keywords()[
        view.source.decoded_offset : view.source.decoded_offset
        + view.source.byte_len
    ]
    assert raw.startswith(b"<View") and raw.endswith(b"</View>")
    assert view.source.sha256 == hashlib.sha256(raw).hexdigest()
    assert result.to_dict() == native


def test_drawing_structure_strict_mode_retains_partial_result():
    with pytest.raises(sldkit.DrawingStructureError) as caught:
        sldkit.decode_drawing_structure_bytes(
            drawing_file(), filename="fixture.SLDDRW", strict=True
        )

    assert caught.value.result.status is sldkit.DrawingStructureStatus.PARTIAL


def test_drawing_structure_rejects_non_drawing_document_kind():
    result = sldkit.decode_drawing_structure_bytes(
        part_file(), filename="fixture.SLDPRT"
    )

    assert result.status is sldkit.DrawingStructureStatus.UNSUPPORTED
    assert result.structure is None
    assert result.diagnostics[-1].code == "drawing.structure_document_kind_unsupported"


def test_modern_properties_keep_present_empty_missing_and_unsupported_distinct():
    model = (
        b'<root><swHeader><swFile swDocType="PART"/></swHeader><swModelList>'
        b'<swModel swConfigurationId="0">'
        b'<swConfiguration swID="0" swName="Default"/>'
        b"</swModel></swModelList></root>"
    )
    properties = (
        b'<root><propertySection name="UserDefinedProperties">'
        b'<property name="present"><lpwstr>value</lpwstr></property>'
        b'<property name="empty"><lpwstr></lpwstr></property>'
        b'<property name="missing"/>'
        b'<property name="opaque"><blob>0102</blob></property>'
        b"</propertySection></root>"
    )
    payload = modern_streams(
        (
            ("swXmlContents/Features", model),
            ("docProps/custom.xml", properties),
        )
    )

    result = sldkit.parse_bytes(payload, filename="fixture.SLDPRT")

    assert result.document is not None
    states = {
        item.name.value: (
            item.value_state,
            None if item.raw_value is None else item.raw_value.value,
        )
        for item in result.document.properties
    }
    assert states == {
        "present": (sldkit.PropertyValueState.PRESENT, "value"),
        "empty": (sldkit.PropertyValueState.EMPTY, ""),
        "missing": (sldkit.PropertyValueState.MISSING, None),
        "opaque": (sldkit.PropertyValueState.UNSUPPORTED_TYPE, "0102"),
    }
    assert [item.code for item in result.diagnostics].count(
        "modern.property_type_unsupported"
    ) == 1
    assert [item.code for item in result.diagnostics].count(
        "modern.property_value_missing"
    ) == 1
    assert result.semantic_coverage is not None
    assert result.to_dict()["document"]["properties"][2]["raw_value"] is None


def test_malformed_modern_xml_remains_a_stream_diagnostic():
    model = b'<root><swHeader><swFile swDocType="PART"/></swHeader></root>'
    payload = modern_streams(
        (
            ("swXmlContents/Features", model),
            ("docProps/custom.xml", b"<root><propertySection>"),
        )
    )

    result = sldkit.parse_bytes(payload, filename="fixture.SLDPRT")

    assert result.status is sldkit.ParseStatus.PARTIAL
    assert result.document is not None
    assert result.document.document_kind.value is sldkit.DocumentKind.PART
    assert any(item.code == "modern.xml_malformed" for item in result.diagnostics)
    assert any(
        item.stream_path == "docProps/custom.xml"
        and item.reason_code == "semantic.stream_malformed"
        for item in result.document.unknown_records
    )


def test_path_entry_points_record_path_source(tmp_path):
    path = tmp_path / "assembly.SLDASM"
    path.write_bytes(modern_file())

    probe = sldkit.probe_file(path)
    parsed = sldkit.parse_file(path)

    assert probe.status is sldkit.ProbeStatus.RECOGNIZED
    assert parsed.document is not None
    assert parsed.document.source.input_kind is sldkit.SourceInputKind.PATH
    assert parsed.document.document_kind.value is sldkit.DocumentKind.ASSEMBLY


def test_inspect_is_deterministic_and_extracts_both_representations():
    payload = modern_file(b"decoded-payload")

    first = sldkit.inspect_bytes(payload)
    second = sldkit.inspect_bytes(payload)

    assert first.status is sldkit.InventoryStatus.COMPLETE
    assert first.to_dict() == second.to_dict()
    assert first.inventory is not None
    entry = first.inventory.entries[0]
    assert entry.checksum is sldkit.ChecksumStatus.VERIFIED
    assert first.coverage.uninterpreted_bytes == 0

    decoded = sldkit.extract_bytes(payload, entry.id)
    stored = sldkit.extract_bytes(payload, entry.id, mode=sldkit.ExtractionMode.STORED)
    assert decoded.result.status is sldkit.ExtractionStatus.EXTRACTED
    assert decoded.data == b"decoded-payload"
    assert stored.data is not None
    assert stored.data != decoded.data


def test_resource_extraction_returns_only_validated_descriptor_range(tmp_path):
    payload = modern_file(b"prefix-preview-suffix", "Contents/Preview")
    inspected = sldkit.inspect_bytes(payload)
    assert inspected.inventory is not None
    entry = inspected.inventory.entries[0]
    resource = sldkit.BinaryResource(
        kind=sldkit.BinaryResourceKind.PREVIEW_PNG,
        entry_id=entry.id,
        stream_path="Contents/Preview",
        decoded_offset=7,
        byte_len=7,
        sha256=hashlib.sha256(b"preview").hexdigest(),
        media_type="image/png",
    )

    extracted = sldkit.extract_resource_bytes(payload, resource)
    assert extracted.result.status is sldkit.ExtractionStatus.EXTRACTED
    assert extracted.result.byte_len == 7
    assert extracted.data == b"preview"

    path = tmp_path / "drawing.SLDDRW"
    path.write_bytes(payload)
    assert sldkit.extract_resource_file(path, resource).data == b"preview"

    wrong_digest = sldkit.BinaryResource(
        kind=resource.kind,
        entry_id=resource.entry_id,
        stream_path=resource.stream_path,
        decoded_offset=resource.decoded_offset,
        byte_len=resource.byte_len,
        sha256="0" * 64,
        media_type=resource.media_type,
    )
    rejected = sldkit.extract_resource_bytes(payload, wrong_digest)
    assert rejected.result.status is sldkit.ExtractionStatus.MALFORMED
    assert rejected.data is None
    assert rejected.result.diagnostics[-1].code == "extract.resource_sha256_mismatch"


def test_zip_inventory_detects_content_without_decoding_opc_semantics():
    payload = stored_zip()
    result = sldkit.inspect_bytes(payload, filename="package.bin")

    assert result.status is sldkit.InventoryStatus.COMPLETE
    assert result.inventory is not None
    assert result.inventory.envelope is sldkit.Envelope.ZIP_OPC
    assert result.inventory.entries[0].path == "docProps/test.xml"
    assert any(item.code == "input.extension_mismatch" for item in result.diagnostics)


def test_full_inventory_rejects_corruption_and_declared_bomb():
    corrupted = bytearray(modern_file())
    corrupted[18] ^= 1
    malformed = sldkit.inspect_bytes(corrupted)
    assert malformed.status is sldkit.InventoryStatus.MALFORMED
    assert any(
        item.code == "modern.checksum_mismatch" and item.offset is not None
        for item in malformed.diagnostics
    )

    bomb = bytearray(modern_file(b"x"))
    bomb[26:30] = (1_000_000).to_bytes(4, "little")
    rejected = sldkit.inspect_bytes(bomb, profile=sldkit.LimitProfile.SERVICE)
    assert rejected.status is sldkit.InventoryStatus.REJECTED
    assert any(item.code == "limit.compression_ratio" for item in rejected.diagnostics)


def test_signature_only_ole2_is_probe_candidate_but_malformed_container():
    result = sldkit.inspect_bytes(OLE2_SIGNATURE)

    assert result.status is sldkit.InventoryStatus.MALFORMED
    assert result.diagnostics[0].code == "ole2.truncated_header"
    assert result.diagnostics[0].offset == len(OLE2_SIGNATURE)


def test_unknown_limit_profile_is_rejected():
    with pytest.raises(ValueError, match="unknown resource-limit profile"):
        sldkit.probe_bytes(OLE2_SIGNATURE, profile="unbounded")


def test_project_scan_resolves_graph_and_matches_native_json(tmp_path):
    private = tmp_path / "private"
    private.mkdir()
    root = tmp_path / "root.SLDASM"
    child = private / "child.SLDPRT"
    root.write_bytes(assembly_file(("private/child.SLDPRT", "PART", "child-1", False)))
    child.write_bytes(part_file())

    result = sldkit.scan_project(
        root,
        project_root=tmp_path,
        configuration="Default",
        profile=sldkit.LimitProfile.SERVICE,
    )
    native = json.loads(
        _core.scan_project_json(
            str(root), str(tmp_path), "Default", [], [], False, "service"
        )
    )

    assert result.status is sldkit.ProjectScanStatus.COMPLETE
    assert result.to_dict() == native
    assert len(result.nodes) == 2
    assert len(result.edges) == 1
    assert result.edges[0].stored_path == "private/child.SLDPRT"
    assert result.edges[0].resolved_path == "private/child.SLDPRT"
    assert result.edges[0].resolution_basis is (
        sldkit.ReferenceResolutionBasis.DOCUMENT_RELATIVE
    )
    assert result.edges[0].traversal_status is (
        sldkit.ReferenceTraversalStatus.FOLLOWED
    )
    assert "private" not in json.dumps(result.compatibility_report.to_dict())


def test_project_scan_rejects_unparseable_root(tmp_path):
    root = tmp_path / "root.SLDASM"
    root.write_bytes(b"not a SolidWorks container")

    result = sldkit.scan_project(root)

    assert result.status is sldkit.ProjectScanStatus.REJECTED
    assert result.nodes[0].parse_status is sldkit.ParseStatus.UNSUPPORTED
    assert any(
        diagnostic.code == "project.root_parse_unavailable"
        for diagnostic in result.diagnostics
    )
