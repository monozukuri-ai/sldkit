from __future__ import annotations

import hashlib
import io
import json
import struct
import zipfile
import zlib
from importlib.metadata import version

import pytest
import sldkit
from sldkit import _core

OLE2_SIGNATURE = bytes.fromhex("d0cf11e0a1b11ae1")
MODERN_MARKER = bytes.fromhex("140006000800")


def test_version_matches_installed_metadata():
    assert sldkit.__version__ == version("sldkit")


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

    result = sldkit.decode_drawing_structure_bytes(payload, filename="fixture.SLDDRW")
    repeated = sldkit.decode_drawing_structure_bytes(payload, filename="fixture.SLDDRW")
    native = json.loads(
        _core.decode_drawing_structure_bytes_json(payload, "fixture.SLDDRW", "desktop")
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
        view.source.decoded_offset : view.source.decoded_offset + view.source.byte_len
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


def virtual_assembly_file(embedded_part: bytes) -> bytes:
    xml = (
        '<root><swHeader><swFile id="0" swDocType="ASSEMBLY"/>'
        '<swFile id="1" swDocType="PART" '
        'swPath="C:\\Temp\\swx1\\VC~~\\root\\Inner^root.sldprt"/>'
        '<swFile id="2" swDocType="PART" swPath="loose.SLDPRT"/>'
        "</swHeader><swModelList>"
        '<swModel id="self" swFileRef="0" swConfigurationId="0" '
        'swBoundingBox="-0.01 -0.02 0 0.03 0.04 0.05">'
        '<swConfiguration swID="0" swName="Default"/>'
        '<swReference swModelRef="m1" swName="Inner^root" swSuppressed="NO" '
        'swIsVirtualComponent="YES" swFlexible="NO" swReferenceNumber="1" '
        'swID="7" swTransform="1 0 0 0 0 1 0 0 0 0 1 0 0.5 0 0 1"/>'
        '<swReference swModelRef="m2" swName="loose-1" swSuppressed="NO" '
        'swReferenceNumber="1" swID="8"/>'
        '<swReference swModelRef="m2" swName="loose-2" swSuppressed="YES" '
        'swReferenceNumber="2" swID="9"/>'
        "</swModel>"
        '<swModel id="m1" swFileRef="1" swConfigurationName="Default"/>'
        '<swModel id="m2" swFileRef="2" swConfigurationName="Default"/>'
        "</swModelList></root>"
    )
    return modern_streams(
        (
            ("swXmlContents/COMPINSTANCETREE", xml.encode()),
            ("VirtualComp/Inner", embedded_part),
        )
    )


def test_parse_exposes_typed_component_facts_and_embedded_documents():
    result = sldkit.parse_bytes(
        virtual_assembly_file(part_file()), filename="root.SLDASM"
    )
    document = result.document
    assert document is not None
    configuration = document.configurations[0]
    assert configuration.bounding_box is not None
    assert configuration.bounding_box.min == ("-0.01", "-0.02", "0")
    assert configuration.bounding_box.length_unit.value == "meter"
    assert configuration.bounding_box.length_unit.origin is sldkit.ValueOrigin.INFERRED
    component = configuration.components[0]
    assert component.is_virtual is not None and component.is_virtual.value is True
    assert component.is_flexible is not None and component.is_flexible.value is False
    assert (
        component.instance_number is not None and component.instance_number.value == 1
    )
    assert component.component_id is not None and component.component_id.value == 7
    assert component.raw_transform is not None
    assert len(component.raw_transform.value) == 16
    assert component.raw_transform.value[12] == "0.5"
    assert configuration.components[1].raw_transform is None
    embedded = document.embedded_documents
    assert [item.name for item in embedded] == ["Inner"]
    assert embedded[0].role is sldkit.EmbeddedDocumentRole.VIRTUAL_COMPONENT
    assert embedded[0].envelope is sldkit.Envelope.MODERN_CHUNK
    extracted = sldkit.extract_bytes(
        virtual_assembly_file(part_file()), embedded[0].entry_id
    )
    assert extracted.data is not None
    nested = sldkit.parse_bytes(extracted.data, filename="Inner.SLDPRT")
    assert nested.document is not None
    assert nested.document.document_kind.value is sldkit.DocumentKind.PART
    assert result.to_dict() == json.loads(
        _core.parse_bytes_json(
            virtual_assembly_file(part_file()), "root.SLDASM", "desktop"
        )
    )


def test_parse_exposes_material_units_feature_records_and_sheet_metadata():
    keywords = (
        b'<Keywords id="7" Name="Part2"><Configuration id="0" Name="Default" '
        b'Type="ConfigurationManager" Material="Steel"/>'
        b'<Extrusion id="30" Name="Boss-Extrude1" Type="Boss-Extrude">'
        b'<Dimension Name="D1">20</Dimension></Extrusion>'
        b'<HoleWizard id="37" Name="CBORE1" Dissectable="true" '
        b'DissectableChildren="36,35" DissectableRoot="true"/></Keywords>'
    )
    system = (
        b'<root><propertySection name="UserDefinedProperties">'
        b'<property name="SW_UnitSystem" pid="21"><i4>4</i4></property>'
        b'<property name="SW_UnitsLinear" pid="22"><i4>3</i4></property>'
        b'<property name="SW_UnitsLinearDecimalPlaces" pid="24"><i4>3</i4></property>'
        b"</propertySection></root>"
    )
    part = modern_streams(
        (
            ("swXmlContents/KeyWords", keywords),
            ("docProps/ISolidWorksInformation.xml", system),
        )
    )
    document = sldkit.parse_bytes(part, filename="part.SLDPRT").document
    assert document is not None
    assert document.configurations[0].material is not None
    assert document.configurations[0].material.value == "Steel"
    units = document.units
    assert units is not None
    assert units.unit_system is not None and units.unit_system.code.value == 4
    assert units.unit_system.name is not None
    assert units.unit_system.name.value == "mmgs"
    assert units.unit_system.name.origin is sldkit.ValueOrigin.INFERRED
    assert units.length is not None and units.length.name is not None
    assert units.length.name.value == "inch"
    assert units.length_decimal_places is not None
    assert units.length_decimal_places.value == 3
    assert units.angle is None
    tags = [
        (record.source_tag, record.source_id) for record in document.feature_records
    ]
    assert tags == [("Extrusion", "30"), ("HoleWizard", "37")]
    assert document.feature_records[0].dimensions[0].raw_value == "20"
    assert document.feature_records[1].dissectable_children == ("36", "35")
    assert (
        document.to_dict()
        == json.loads(_core.parse_bytes_json(part, "part.SLDPRT", "desktop"))[
            "document"
        ]
    )

    sheet_keywords = (
        b'<Keywords><Sheet id="4" Name="Sheet1" Type="Sheet">'
        b'<PaperSize id="" Description="8">A3</PaperSize>'
        b'<ScaleDenominator id="">2.000000</ScaleDenominator>'
        b'<ScaleNumerator id="">1.000000</ScaleNumerator>'
        b'<TemplateName id="">C:\\templates\\a3.slddrt</TemplateName>'
        b'<View id="2" Name="Front" Description="Default">part.sldprt</View>'
        b"</Sheet></Keywords>"
    )
    sheet_system = (
        b'<root><propertySection name="UserDefinedProperties">'
        b'<property name="SW-All Sheet Format Data" pid="9"><lpstr>'
        b"Sheet1@8.000000@0.420000@0.297000@1.000000@2.000000@0.000000@"
        b"C:\\templates\\a3.slddrt@8.000000@1.000000@@</lpstr></property>"
        b"</propertySection></root>"
    )
    drawing = modern_streams(
        (
            ("swXmlContents/KeyWords", sheet_keywords),
            ("docProps/ISolidWorksInformation.xml", sheet_system),
        )
    )
    sheet = sldkit.parse_bytes(drawing, filename="d.SLDDRW").document.sheets[0]
    assert sheet.paper_size is not None and sheet.paper_size.value == "A3"
    assert sheet.paper_size_code is not None and sheet.paper_size_code.value == 8
    assert sheet.paper_size_name is not None and sheet.paper_size_name.value == "a3"
    assert sheet.scale_numerator is not None
    assert sheet.scale_numerator.value == "1.000000"
    assert sheet.scale_denominator is not None
    assert sheet.scale_denominator.value == "2.000000"
    assert sheet.sheet_format_path is not None
    assert sheet.sheet_format_path.value == "C:\\templates\\a3.slddrt"
    assert sheet.width is not None and sheet.width.value == "0.420000"
    assert sheet.height is not None and sheet.height.value == "0.297000"
    assert sheet.format_data is not None and len(sheet.format_data.fields) == 10
    assert (
        sheet.to_dict()
        == json.loads(_core.parse_bytes_json(drawing, "d.SLDDRW", "desktop"))[
            "document"
        ]["sheets"][0]
    )


def test_project_scan_resolves_virtual_components_to_embedded_nodes(tmp_path):
    root = tmp_path / "root.SLDASM"
    root.write_bytes(virtual_assembly_file(part_file()))
    (tmp_path / "loose.SLDPRT").write_bytes(part_file())

    result = sldkit.scan_project(root, project_root=tmp_path, configuration="Default")

    assert result.status is sldkit.ProjectScanStatus.COMPLETE
    virtual = result.edges[0]
    assert virtual.is_virtual is True
    assert virtual.instance_number == 1
    assert virtual.resolution_basis is (
        sldkit.ReferenceResolutionBasis.EMBEDDED_VIRTUAL_COMPONENT
    )
    assert virtual.resolved_path == "root.SLDASM#VirtualComp/Inner"
    embedded = result.node_by_path("root.SLDASM#VirtualComp/Inner")
    assert embedded is not None
    assert embedded.document_kind is sldkit.DocumentKind.PART
    assert embedded.embedded_source is not None
    assert embedded.embedded_source.container_node_id == result.root_node_id
    assert embedded.embedded_source.stream_path == "VirtualComp/Inner"
    assert result.to_dict() == json.loads(
        _core.scan_project_json(
            str(root), str(tmp_path), "Default", [], [], False, "desktop"
        )
    )


def test_index_project_supports_reverse_lookup(tmp_path):
    (tmp_path / "parts").mkdir()
    (tmp_path / "parts" / "leaf.SLDPRT").write_bytes(part_file())
    (tmp_path / "top.SLDASM").write_bytes(
        assembly_file(("parts/leaf.SLDPRT", "PART", "leaf-1", False))
    )
    keywords = (
        b'<Keywords><Sheet Type="Sheet" Name="Sheet1" id="s1">'
        b'<View Name="Front" Description="Default" id="v1">parts/leaf.SLDPRT</View>'
        b"</Sheet></Keywords>"
    )
    (tmp_path / "leaf.SLDDRW").write_bytes(
        modern_file(keywords, "swXmlContents/KeyWords")
    )

    result = sldkit.index_project(tmp_path, profile="service")
    native = json.loads(
        _core.index_project_json(str(tmp_path), [], [], False, "service")
    )

    assert result.status is sldkit.ProjectScanStatus.COMPLETE
    assert result.root_node_id is None
    assert result.to_dict() == native
    assert all(node.is_root for node in result.nodes)
    leaf = result.node_by_path("parts/leaf.SLDPRT")
    assert leaf is not None
    assert {edge.kind for edge in result.edges_to(leaf.id)} == {
        sldkit.ReferenceKind.ASSEMBLY_COMPONENT,
        sldkit.ReferenceKind.DRAWING_VIEW,
    }
    drawings = result.referencing_nodes(leaf.id, kind=sldkit.ReferenceKind.DRAWING_VIEW)
    assert [node.path for node in drawings] == ["leaf.SLDDRW"]


def test_build_bom_aggregates_quantities_and_requires_root_configuration(tmp_path):
    (tmp_path / "bolt.SLDPRT").write_bytes(part_file())
    (tmp_path / "sub.SLDASM").write_bytes(
        assembly_file(
            ("bolt.SLDPRT", "PART", "bolt-1", False),
            ("bolt.SLDPRT", "PART", "bolt-2", False),
        )
    )
    root = tmp_path / "root.SLDASM"
    root.write_bytes(
        assembly_file(
            ("sub.SLDASM", "ASSEMBLY", "sub-1", False),
            ("sub.SLDASM", "ASSEMBLY", "sub-2", False),
            ("bolt.SLDPRT", "PART", "bolt-3", True),
            ("gone.SLDPRT", "PART", "gone-1", False),
        )
    )

    graph = sldkit.scan_project(root, project_root=tmp_path, configuration="Default")
    bom = sldkit.build_bom(graph)
    native = json.loads(
        _core.build_bom_json(
            json.dumps(graph.to_dict(), separators=(",", ":"), sort_keys=True),
            json.dumps(sldkit.BomOptions().to_dict(), sort_keys=True),
        )
    )

    assert bom.to_dict() == native
    assert bom.status is sldkit.BomStatus.PARTIAL
    assert bom.root_configuration == "Default"
    assert bom.summary.occurrence_count == 1 + 4 + 2 * 2
    by_name = {item.display_name: item for item in bom.items}
    assert by_name["sub.SLDASM"].quantity == 2
    assert by_name["bolt.SLDPRT"].quantity == 4
    assert by_name["bolt.SLDPRT"].suppressed_quantity == 1
    assert by_name["gone.SLDPRT"].key.node_id is None
    assert by_name["gone.SLDPRT"].quantity == 1
    assert [item.display_name for item in bom.part_items()] == ["bolt.SLDPRT"]
    assert by_name["gone.SLDPRT"].document_kind is None
    root_children = bom.children_of(bom.occurrences[0].id)
    assert [item.instance_name for item in root_children] == [
        "sub-1",
        "sub-2",
        "bolt-3",
        "gone-1",
    ]
    assert root_children[2].children_state is sldkit.BomChildrenState.SUPPRESSED
    assert root_children[3].children_state is sldkit.BomChildrenState.UNRESOLVED

    inclusive = sldkit.build_bom(graph, include_suppressed=True)
    assert {item.display_name: item.quantity for item in inclusive.items}[
        "bolt.SLDPRT"
    ] == 5

    multi = sldkit.scan_project(root, project_root=tmp_path)
    single = sldkit.build_bom(multi)
    assert single.root_configuration == "Default"
    assert single.status is sldkit.BomStatus.PARTIAL
    assert single.summary.children_state_counts["unresolved"] == 1
    renamed = sldkit.build_bom(multi, root_configuration="Nope")
    assert renamed.status is sldkit.BomStatus.REJECTED
    assert any(
        diagnostic.code == "bom.root_configuration_missing"
        for diagnostic in renamed.diagnostics
    )


def test_geometry_evaluation_is_typed_and_matches_native_json():
    payload = part_file() + modern_frame(
        "Contents/DisplayLists", display_list_payload()
    )

    result = sldkit.evaluate_geometry_bytes(payload, filename="cache.SLDPRT")
    native = json.loads(
        _core.evaluate_geometry_bytes_json(payload, "cache.SLDPRT", "desktop", "{}")
    )

    assert result.to_dict() == native
    assert sldkit.GeometryEvaluationResult.from_dict(native) == result
    assert result.status is sldkit.GeometryEvaluationStatus.EVALUATED
    evaluation = result.evaluation
    assert evaluation is not None
    assert evaluation.options == sldkit.GeometryEvaluationOptions()
    assert evaluation.edges == ()
    assert len(evaluation.meshes) == 1
    mesh = evaluation.meshes[0]
    assert mesh.triangle_count == 1
    assert mesh.is_closed is False
    assert mesh.signed_volume_mm3 is None
    assert mesh.area_mm2 == pytest.approx(500_000.0)
    assert mesh.boundary_edges == ((0, 1), (0, 2), (1, 2))
    assert evaluation.bounding_box is not None
    assert evaluation.bounding_box.method is sldkit.BoundingBoxMethod.TESSELLATION
    assert evaluation.bounding_box.exact is False
    assert evaluation.bounding_box.bounds.size == (1000.0, 1000.0, 0.0)
    assert evaluation.summary.mesh_count == 1
    assert {loss.code for loss in evaluation.losses} >= {"evaluation.mesh_not_closed"}

    decoded = sldkit.decode_geometry_bytes(payload, filename="cache.SLDPRT")
    from_result = sldkit.evaluate_geometry(decoded, mesh_feature_angle_deg=45.0)
    assert from_result.evaluation is not None
    assert from_result.evaluation.options.mesh_feature_angle_deg == 45.0
    assert from_result.evaluation.meshes[0].feature_angle_deg == 45.0
    assert from_result.evaluation.summary == evaluation.summary

    strict = sldkit.evaluate_geometry_bytes(
        payload, filename="cache.SLDPRT", strict=True
    )
    assert strict == result
    with pytest.raises(TypeError):
        sldkit.evaluate_geometry_bytes(payload, filename="cache.SLDPRT", bogus=1)

    unavailable = sldkit.evaluate_geometry_bytes(assembly_file(), filename="x.SLDASM")
    assert unavailable.status is sldkit.GeometryEvaluationStatus.UNAVAILABLE
    assert unavailable.evaluation is None
    rejected = sldkit.evaluate_geometry_bytes(payload, chord_tolerance_mm=-1.0)
    assert rejected.status is sldkit.GeometryEvaluationStatus.REJECTED
    with pytest.raises(sldkit.GeometryEvaluationError) as caught:
        sldkit.evaluate_geometry_bytes(payload, chord_tolerance_mm=-1.0, strict=True)
    assert caught.value.result.status is sldkit.GeometryEvaluationStatus.REJECTED
