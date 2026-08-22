from __future__ import annotations

import json
import struct
import zlib

from sldkit.cli import main

OLE2_SIGNATURE = bytes.fromhex("d0cf11e0a1b11ae1")
MODERN_MARKER = bytes.fromhex("140006000800")


def modern_file(
    payload: bytes = b"payload", name_value: bytes = b"Contents/Test"
) -> bytes:
    compressor = zlib.compressobj(wbits=-zlib.MAX_WBITS)
    compressed = compressor.compress(payload) + compressor.flush()
    name = bytes(
        ((value << 4) & 0xF0) | (value >> 4) for value in name_value
    )
    return b"".join(
        (
            b"SLDK",
            (4).to_bytes(4, "big"),
            MODERN_MARKER,
            struct.pack(
                "<IIIII",
                7,
                zlib.crc32(payload),
                len(compressed),
                len(payload),
                len(name),
            ),
            name,
            compressed,
        )
    )


def test_probe_cli_prints_machine_readable_result(tmp_path, capsys):
    path = tmp_path / "part.SLDPRT"
    path.write_bytes(OLE2_SIGNATURE)

    exit_code = main(["probe", str(path)])
    output = json.loads(capsys.readouterr().out)

    assert exit_code == 0
    assert output["status"] == "recognized"
    assert output["envelope"] == "ole2_cfb"


def test_parse_cli_accepts_modern_partial_result(tmp_path, capsys):
    path = tmp_path / "part.SLDPRT"
    path.write_bytes(modern_file())

    exit_code = main(["parse", str(path)])
    output = json.loads(capsys.readouterr().out)

    assert exit_code == 0
    assert output["status"] == "partial"
    assert output["document"]["document_kind"]["value"] == "part"


def test_inspect_and_extract_cli_use_inventory_entry_id(tmp_path, capsys):
    path = tmp_path / "part.SLDPRT"
    output_path = tmp_path / "payload.bin"
    path.write_bytes(modern_file(b"decoded"))

    inspect_code = main(["inspect", str(path)])
    inventory = json.loads(capsys.readouterr().out)
    entry_id = inventory["inventory"]["entries"][0]["id"]
    extract_code = main(["extract", str(path), entry_id, str(output_path)])
    extraction = json.loads(capsys.readouterr().out)

    assert inspect_code == 0
    assert extract_code == 0
    assert extraction["status"] == "extracted"
    assert output_path.read_bytes() == b"decoded"


def test_probe_cli_uses_distinct_failure_code_for_malformed(tmp_path, capsys):
    path = tmp_path / "empty.SLDPRT"
    path.write_bytes(b"")

    exit_code = main(["probe", str(path)])
    output = json.loads(capsys.readouterr().out)

    assert exit_code == 2
    assert output["status"] == "malformed"


def test_scan_cli_can_emit_path_free_summary(tmp_path, capsys):
    root = tmp_path / "root.SLDASM"
    child = tmp_path / "child.SLDPRT"
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
    root.write_bytes(modern_file(assembly_xml, b"swXmlContents/COMPINSTANCETREE"))
    child.write_bytes(modern_file(part_xml, b"swXmlContents/Features"))

    exit_code = main(
        [
            "scan",
            str(root),
            "--project-root",
            str(tmp_path),
            "--configuration",
            "Default",
            "--summary",
            "--limits",
            "service",
        ]
    )
    output = json.loads(capsys.readouterr().out)

    assert exit_code == 0
    assert output["status"] == "complete"
    assert output["node_count"] == 2
    assert output["edge_count"] == 1
    assert "root.SLDASM" not in json.dumps(output)
