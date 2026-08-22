#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path
from typing import Any

import sldkit

VERSION_PATH = re.compile(r"(?:^|/)_MO_VERSION_(\d+)(?:/|$)")


def _canonical(value: dict[str, Any]) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()


def _case(path: Path, profile: str) -> tuple[dict[str, Any], bool]:
    source = path.read_bytes()
    first = sldkit.inspect_bytes(source, filename=path.name, profile=profile)
    second = sldkit.inspect_bytes(source, filename=path.name, profile=profile)
    first_json = _canonical(first.to_dict())
    second_json = _canonical(second.to_dict())
    deterministic = first_json == second_json
    inventory = first.inventory

    versions: set[int] = set()
    if inventory is not None:
        for entry in inventory.entries:
            if entry.path is not None and (match := VERSION_PATH.search(entry.path)):
                versions.add(int(match.group(1)))

    extraction: dict[str, Any] | None = None
    extraction_ok = True
    if inventory is not None:
        candidate = next(
            (
                entry
                for entry in inventory.entries
                if entry.decoded_sha256 is not None
                and entry.stored_sha256 is not None
            ),
            None,
        )
        if candidate is not None:
            decoded = sldkit.extract_bytes(
                source,
                candidate.id,
                mode=sldkit.ExtractionMode.DECODED,
                profile=profile,
            )
            stored = sldkit.extract_bytes(
                source, candidate.id, mode=sldkit.ExtractionMode.STORED, profile=profile
            )
            decoded_hash = (
                None
                if decoded.data is None
                else hashlib.sha256(decoded.data).hexdigest()
            )
            stored_hash = (
                None if stored.data is None else hashlib.sha256(stored.data).hexdigest()
            )
            extraction_ok = (
                decoded.result.status is sldkit.ExtractionStatus.EXTRACTED
                and stored.result.status is sldkit.ExtractionStatus.EXTRACTED
                and decoded_hash == candidate.decoded_sha256
                and stored_hash == candidate.stored_sha256
            )
            extraction = {
                "entry_id": candidate.id,
                "decoded_sha256": decoded_hash,
                "stored_sha256": stored_hash,
                "verified": extraction_ok,
            }

    complete = first.status is sldkit.InventoryStatus.COMPLETE
    record = {
        "source_name": path.name,
        "source_bytes": len(source),
        "source_sha256": hashlib.sha256(source).hexdigest(),
        "status": first.status.value,
        "envelope": None if inventory is None else inventory.envelope.value,
        "format_version": None if inventory is None else inventory.format_version,
        "entry_count": 0 if inventory is None else len(inventory.entries),
        "version_path_evidence": sorted(versions),
        "coverage": first.coverage.to_dict(),
        "diagnostic_codes": [item.code for item in first.diagnostics],
        "inventory_json_sha256": hashlib.sha256(first_json).hexdigest(),
        "deterministic": deterministic,
        "extraction": extraction,
    }
    return record, complete and deterministic and extraction_ok


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Validate container inventory determinism and extraction "
            "against local files"
        )
    )
    parser.add_argument("paths", nargs="+", type=Path)
    parser.add_argument("--profile", choices=("desktop", "service"), default="service")
    args = parser.parse_args()

    records = []
    passed = True
    for path in args.paths:
        record, case_passed = _case(path, args.profile)
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
