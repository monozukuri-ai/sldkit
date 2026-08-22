#!/usr/bin/env python3
"""Validate semantic determinism and Rust/Python JSON parity."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from typing import Any

import sldkit
from sldkit import _core


def canonical(value: dict[str, Any]) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()


def rust_result(path: Path, executable: Path, profile: str) -> dict[str, Any]:
    completed = subprocess.run(
        [str(executable), "parse", str(path), "--limits", profile],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(
            f"Rust parser failed with exit {completed.returncode}: {detail}"
        )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise TypeError("Rust parser returned non-object JSON")
    return value


def coverage_is_consistent(coverage: dict[str, Any] | None) -> bool:
    if coverage is None:
        return False
    stream_sum = sum(
        int(coverage[name])
        for name in (
            "fully_interpreted_streams",
            "partially_interpreted_streams",
            "uninterpreted_streams",
            "malformed_streams",
        )
    )
    byte_sum = sum(
        int(coverage[name])
        for name in (
            "fully_interpreted_bytes",
            "partially_interpreted_bytes",
            "uninterpreted_bytes",
            "malformed_bytes",
        )
    )
    return (
        stream_sum == int(coverage["decoded_streams_total"])
        and byte_sum == int(coverage["decoded_bytes_total"])
    )


def case(path: Path, executable: Path, profile: str) -> tuple[dict[str, Any], bool]:
    first = sldkit.parse_file(path, profile=profile).to_dict()
    second = sldkit.parse_file(path, profile=profile).to_dict()
    native = json.loads(_core.parse_file_json(str(path), profile))
    rust = rust_result(path, executable, profile)
    document = first.get("document")
    if not isinstance(document, dict):
        raise RuntimeError(f"semantic parser returned no document for {path}")

    deterministic = canonical(first) == canonical(second)
    native_parity = canonical(first) == canonical(native)
    rust_parity = canonical(first) == canonical(rust)
    semantic_coverage = first.get("semantic_coverage")
    coverage_consistent = coverage_is_consistent(semantic_coverage)
    passed = (
        first["status"] == "partial"
        and deterministic
        and native_parity
        and rust_parity
        and coverage_consistent
    )

    configurations = document.get("configurations", [])
    sheets = document.get("sheets", [])
    property_states: dict[str, int] = {}
    for prop in document.get("properties", []):
        state = str(prop["value_state"])
        property_states[state] = property_states.get(state, 0) + 1
    record = {
        "source_name": path.name,
        "source_bytes": path.stat().st_size,
        "source_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "status": first["status"],
        "document_kind": document["document_kind"],
        "internal_version": document.get("internal_version"),
        "configuration_indices": [
            config["index"]["value"] for config in configurations
        ],
        "configuration_names": [
            None if config.get("name") is None else config["name"]["value"]
            for config in configurations
        ],
        "component_counts": [
            len(config.get("components", [])) for config in configurations
        ],
        "property_states": property_states,
        "reference_count": len(document.get("references", [])),
        "sheet_names": [
            None if sheet.get("name") is None else sheet["name"]["value"]
            for sheet in sheets
        ],
        "view_counts": [len(sheet.get("views", [])) for sheet in sheets],
        "has_document_preview": document.get("preview") is not None,
        "configuration_mass_properties": [
            config.get("mass_properties") is not None for config in configurations
        ],
        "unknown_record_count": len(document.get("unknown_records", [])),
        "semantic_coverage": semantic_coverage,
        "diagnostic_codes": [item["code"] for item in first["diagnostics"]],
        "document_json_sha256": hashlib.sha256(canonical(first)).hexdigest(),
        "deterministic": deterministic,
        "python_native_json_parity": native_parity,
        "rust_cli_json_parity": rust_parity,
        "semantic_coverage_consistent": coverage_consistent,
    }
    return record, passed


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rust-cli", type=Path, required=True)
    parser.add_argument("--profile", choices=("desktop", "service"), default="service")
    parser.add_argument("paths", nargs="+", type=Path)
    args = parser.parse_args()

    records = []
    passed = True
    for path in args.paths:
        record, case_passed = case(path, args.rust_cli, args.profile)
        records.append(record)
        passed = passed and case_passed
    print(
        json.dumps(
            {"schema_version": 1, "passed": passed, "cases": records},
            ensure_ascii=False,
            indent=2,
            sort_keys=True,
        )
    )
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
