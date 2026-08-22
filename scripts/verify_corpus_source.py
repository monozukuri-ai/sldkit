#!/usr/bin/env python3
"""Verify a pinned external checkout against the corpus manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path
from typing import Any

ROOT = Path(__file__).parents[1]
CORPUS = ROOT / "corpus"
GIT_COMMIT = re.compile(r"[0-9a-f]{40}")


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_revision(checkout: Path) -> str:
    completed = subprocess.run(
        ["git", "-C", str(checkout), "rev-parse", "HEAD"],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(f"cannot read checkout revision: {detail}")
    return completed.stdout.strip()


def load_source(source_id: str) -> dict[str, Any]:
    lock = json.loads((CORPUS / "sources.lock.json").read_text(encoding="utf-8"))
    matches = [source for source in lock["sources"] if source["source_id"] == source_id]
    if len(matches) != 1:
        raise ValueError(f"source_id must match exactly one source: {source_id}")
    return matches[0]


def load_artifacts(source_id: str) -> list[dict[str, Any]]:
    entries = [
        json.loads(line)
        for line in (CORPUS / "manifest.jsonl").read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    artifacts = [
        entry
        for entry in entries
        if entry["source_id"] == source_id and entry.get("source_path") is not None
    ]
    if not artifacts:
        raise ValueError(f"source has no artifacts with source_path: {source_id}")
    return artifacts


def verify(source_id: str, checkout: Path) -> dict[str, Any]:
    source = load_source(source_id)
    artifacts = load_artifacts(source_id)
    checkout_root = checkout.resolve(strict=True)
    expected_revision = source["revision"]
    actual_revision = None
    revision_matches = None
    if GIT_COMMIT.fullmatch(expected_revision):
        actual_revision = git_revision(checkout_root)
        revision_matches = actual_revision == expected_revision

    records = []
    for artifact in artifacts:
        source_path = artifact["source_path"]
        path = (checkout_root / source_path).resolve()
        confined = path == checkout_root or checkout_root in path.parents
        exists = confined and path.is_file()
        byte_size = path.stat().st_size if exists else None
        sha256 = file_sha256(path) if exists else None
        records.append(
            {
                "artifact_id": artifact["artifact_id"],
                "source_path": source_path,
                "exists": exists,
                "byte_size": byte_size,
                "byte_size_matches": byte_size == artifact["byte_size"],
                "sha256": sha256,
                "sha256_matches": sha256 == artifact["sha256"],
            }
        )

    passed = revision_matches is not False and all(
        record["exists"] and record["byte_size_matches"] and record["sha256_matches"]
        for record in records
    )
    return {
        "schema_version": 1,
        "source_id": source_id,
        "expected_revision": expected_revision,
        "actual_revision": actual_revision,
        "revision_matches": revision_matches,
        "artifacts": records,
        "passed": passed,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--source-id", required=True)
    parser.add_argument("checkout", type=Path)
    args = parser.parse_args()

    try:
        result = verify(args.source_id, args.checkout)
    except (OSError, RuntimeError, ValueError, json.JSONDecodeError) as error:
        print(json.dumps({"schema_version": 1, "passed": False, "error": str(error)}))
        return 1
    print(json.dumps(result, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
