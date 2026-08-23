from __future__ import annotations

import hashlib
import json
from pathlib import Path

from jsonschema import Draft202012Validator

ROOT = Path(__file__).parents[1]
CORPUS = ROOT / "corpus"
CAD_SUFFIXES = {".sldprt", ".sldasm", ".slddrw"}


def _load_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def test_corpus_schemas_and_source_lock_are_valid():
    manifest_schema = _load_json(CORPUS / "manifest.schema.json")
    source_schema = _load_json(CORPUS / "source-lock.schema.json")
    Draft202012Validator.check_schema(manifest_schema)
    Draft202012Validator.check_schema(source_schema)
    Draft202012Validator(source_schema).validate(
        _load_json(CORPUS / "sources.lock.json")
    )


def test_manifest_sources_dependencies_and_audited_public_contract():
    source_lock = _load_json(CORPUS / "sources.lock.json")
    source_ids = [source["source_id"] for source in source_lock["sources"]]
    assert len(source_ids) == len(set(source_ids))

    entries = [
        json.loads(line)
        for line in (CORPUS / "manifest.jsonl").read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    artifact_ids = {entry["artifact_id"] for entry in entries}
    source_paths: set[tuple[str, str]] = set()
    for entry in entries:
        if entry["source_id"] is not None:
            assert entry["source_id"] in source_ids
        assert set(entry["dependencies"]) <= artifact_ids

        source_path = entry.get("source_path")
        if source_path is not None:
            assert entry["source_id"] is not None
            key = (entry["source_id"], source_path)
            assert key not in source_paths
            source_paths.add(key)

        if entry["corpus_class"] == "audited_public":
            assert entry["redistribution"] == "allowed"
            assert source_path is not None
            assert entry.get("reference_closure") is not None


def test_each_manifest_line_is_valid_and_matches_local_artifact():
    schema = _load_json(CORPUS / "manifest.schema.json")
    validator = Draft202012Validator(schema)
    entries = []
    for line_number, line in enumerate(
        (CORPUS / "manifest.jsonl").read_text(encoding="utf-8").splitlines(),
        start=1,
    ):
        if not line.strip():
            continue
        entry = json.loads(line)
        errors = sorted(validator.iter_errors(entry), key=lambda error: error.json_path)
        assert not errors, f"manifest line {line_number}: {errors}"
        entries.append(entry)

        if entry["local_path"] is not None:
            artifact = ROOT / entry["local_path"]
            assert artifact.is_file()
            payload = artifact.read_bytes()
            assert len(payload) == entry["byte_size"]
            assert hashlib.sha256(payload).hexdigest() == entry["sha256"]

    ids = [entry["artifact_id"] for entry in entries]
    assert len(ids) == len(set(ids))


def test_no_unmanifested_cad_binary_is_vendored_in_corpus():
    vendored = {
        path.relative_to(ROOT).as_posix()
        for path in CORPUS.rglob("*")
        if path.is_file()
        and path.suffix.lower() in CAD_SUFFIXES
        and path.relative_to(CORPUS).parts[0] not in {"cache", "external"}
    }
    manifested = {
        entry["local_path"]
        for line in (CORPUS / "manifest.jsonl").read_text(encoding="utf-8").splitlines()
        if line.strip()
        for entry in [json.loads(line)]
        if entry["local_path"] is not None
    }
    assert vendored <= manifested
