#!/usr/bin/env python3
from __future__ import annotations

import io
import struct
import tempfile
import zipfile
import zlib
from importlib import metadata
from pathlib import Path

import sldkit

OLE2_SIGNATURE = bytes.fromhex("d0cf11e0a1b11ae1")
MODERN_MARKER = bytes.fromhex("140006000800")


def modern_stream(name: str, payload: bytes) -> bytes:
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

assert metadata.version("sldkit") == sldkit.__version__

probe = sldkit.probe_bytes(OLE2_SIGNATURE)
assert probe.status is sldkit.ProbeStatus.RECOGNIZED
assert probe.envelope is sldkit.Envelope.OLE2_CFB

unsupported = sldkit.parse_bytes(b"unknown input")
malformed = sldkit.parse_bytes(b"")
assert unsupported.status is sldkit.ParseStatus.UNSUPPORTED
assert malformed.status is sldkit.ParseStatus.MALFORMED
assert unsupported.diagnostics[0].kind is sldkit.DiagnosticKind.UNSUPPORTED
assert malformed.diagnostics[0].kind is sldkit.DiagnosticKind.MALFORMED

archive_bytes = io.BytesIO()
with zipfile.ZipFile(archive_bytes, "w", compression=zipfile.ZIP_STORED) as archive:
    archive.writestr("payload.bin", b"isolated-payload")

inventory = sldkit.inspect_bytes(archive_bytes.getvalue())
assert inventory.status is sldkit.InventoryStatus.COMPLETE
assert inventory.inventory is not None
assert inventory.inventory.envelope is sldkit.Envelope.ZIP_OPC
entry = inventory.inventory.entries[0]
extracted = sldkit.extract_bytes(archive_bytes.getvalue(), entry.id)
assert extracted.result.status is sldkit.ExtractionStatus.EXTRACTED
assert extracted.data == b"isolated-payload"

features = b'<root><swHeader><swFile swDocType="PART"/></swHeader></root>'
properties = (
    b'<root><propertySection name="UserDefinedProperties">'
    b'<property name="present"><lpwstr>value</lpwstr></property>'
    b'<property name="empty"><lpwstr></lpwstr></property>'
    b"</propertySection></root>"
)
modern = b"SLDK" + (4).to_bytes(4, "big")
modern += modern_stream("swXmlContents/Features", features)
modern += modern_stream("docProps/custom.xml", properties)
parsed = sldkit.parse_bytes(modern, filename="fixture.SLDPRT")
assert parsed.status is sldkit.ParseStatus.PARTIAL
assert parsed.document is not None
assert parsed.document.document_kind.value is sldkit.DocumentKind.PART
assert {
    item.name.value: item.value_state for item in parsed.document.properties
} == {
    "present": sldkit.PropertyValueState.PRESENT,
    "empty": sldkit.PropertyValueState.EMPTY,
}

assembly_xml = (
    b'<root><swHeader><swFile id="0" swDocType="ASSEMBLY"/>'
    b'<swFile id="1" swDocType="PART" swPath="child.SLDPRT"/>'
    b'</swHeader><swModelList><swModel id="self" swFileRef="0" '
    b'swConfigurationId="0"><swConfiguration swID="0" swName="Default"/>'
    b'<swReference swModelRef="child" swName="child-1" swSuppressed="NO"/>'
    b'</swModel><swModel id="child" swFileRef="1" '
    b'swConfigurationName="Default"/></swModelList></root>'
)
part_xml = (
    b'<root><swHeader><swFile id="0" swDocType="PART"/></swHeader>'
    b'<swModelList><swModel id="self" swFileRef="0" '
    b'swConfigurationId="0"><swConfiguration swID="0" '
    b'swName="Default"/></swModel></swModelList></root>'
)
with tempfile.TemporaryDirectory() as directory:
    project_root = Path(directory)
    assembly_path = project_root / "root.SLDASM"
    part_path = project_root / "child.SLDPRT"
    assembly_path.write_bytes(
        b"SLDK"
        + (4).to_bytes(4, "big")
        + modern_stream("swXmlContents/COMPINSTANCETREE", assembly_xml)
    )
    part_path.write_bytes(
        b"SLDK"
        + (4).to_bytes(4, "big")
        + modern_stream("swXmlContents/Features", part_xml)
    )
    graph = sldkit.scan_project(
        assembly_path,
        project_root=project_root,
        configuration="Default",
        profile=sldkit.LimitProfile.SERVICE,
    )
    assert graph.status is sldkit.ProjectScanStatus.COMPLETE
    assert graph.compatibility_report.node_count == 2
    assert graph.compatibility_report.edge_count == 1
    assert graph.edges[0].resolved_path == "child.SLDPRT"

print("isolated sldkit smoke passed")
