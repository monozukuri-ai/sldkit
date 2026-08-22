#!/usr/bin/env python3
"""Validate project graph determinism, API parity, and an optional closure."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
from collections import defaultdict
from collections.abc import Iterable, Mapping
from pathlib import Path
from typing import Any

import sldkit
from sldkit import _core

PATH_KEYS = {
    "candidate_paths",
    "path",
    "resolved_path",
    "source_sha256",
    "stored_path",
}


def canonical(value: Mapping[str, Any]) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode()


def normalized_label(value: str) -> str:
    normalized = value.replace("\\", "/")
    parts = [part for part in normalized.split("/") if part not in {"", "."}]
    if not parts or ".." in parts or normalized.startswith("/"):
        raise ValueError(f"closure path must be a project-relative label: {value!r}")
    if len(parts[0]) == 2 and parts[0][1] == ":":
        raise ValueError(f"closure path must not have a drive prefix: {value!r}")
    label = "/".join(parts)
    if not label.casefold().endswith((".sldprt", ".sldasm", ".slddrw")):
        raise ValueError(f"closure path must name a SolidWorks document: {value!r}")
    return label


def closure_difference(
    graph: Mapping[str, Any], expected_paths: Iterable[str]
) -> dict[str, Any]:
    graph_status = str(graph.get("status", "unknown"))
    expected = sorted({normalized_label(path) for path in expected_paths})
    actual = sorted(
        normalized_label(str(node["path"])) for node in graph.get("nodes", [])
    )
    expected_by_fold: dict[str, list[str]] = defaultdict(list)
    for path in expected:
        expected_by_fold[path.casefold()].append(path)
    actual_by_fold: dict[str, list[str]] = defaultdict(list)
    for path in actual:
        actual_by_fold[path.casefold()].append(path)
    expected_case_collisions = [
        paths for paths in expected_by_fold.values() if len(paths) > 1
    ]
    graph_case_collisions = [
        paths for paths in actual_by_fold.values() if len(paths) > 1
    ]
    missing = [
        path
        for folded, paths in expected_by_fold.items()
        if folded not in actual_by_fold
        for path in paths
    ]
    unexpected = [
        path for folded, paths in actual_by_fold.items()
        if folded not in expected_by_fold
        for path in paths
    ]
    unresolved = [
        {
            "edge_id": str(edge["id"]),
            "resolution_status": str(edge["resolution_status"]),
        }
        for edge in graph.get("edges", [])
        if edge.get("resolution_status") != "resolved"
    ]
    matches = (
        graph_status == "complete"
        and not missing
        and not unexpected
        and not unresolved
        and not expected_case_collisions
        and not graph_case_collisions
    )
    return {
        "schema_version": 1,
        "status": "match" if matches else "mismatch",
        "graph_status": graph_status,
        "expected_node_count": len(expected),
        "graph_node_count": len(actual),
        "missing_from_graph": missing,
        "unexpected_in_graph": unexpected,
        "expected_case_collisions": expected_case_collisions,
        "graph_case_collisions": graph_case_collisions,
        "unresolved_edges": unresolved,
    }


def contains_path_fields(value: Any) -> bool:
    if isinstance(value, Mapping):
        return any(key in PATH_KEYS for key in value) or any(
            contains_path_fields(item) for item in value.values()
        )
    if isinstance(value, list):
        return any(contains_path_fields(item) for item in value)
    return False


def rust_result(
    executable: Path,
    root_document: Path,
    project_root: Path,
    configuration: str | None,
    search_directories: list[Path],
    windows_prefix_mappings: list[tuple[str, Path]],
    follow_suppressed: bool,
    profile: str,
) -> dict[str, Any]:
    command = [
        str(executable),
        "scan",
        str(root_document),
        "--project-root",
        str(project_root),
        "--limits",
        profile,
    ]
    if configuration is not None:
        command.extend(("--configuration", configuration))
    for directory in search_directories:
        command.extend(("--search-dir", str(directory)))
    for source, target in windows_prefix_mappings:
        command.extend(("--windows-prefix-map", f"{source}={target}"))
    if follow_suppressed:
        command.append("--follow-suppressed")
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    if completed.returncode not in {0, 1, 2} or not completed.stdout.strip():
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise RuntimeError(
            f"Rust project scan failed with exit {completed.returncode}: {detail}"
        )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise TypeError("Rust project scan returned non-object JSON")
    return value


def validate(
    root_document: Path,
    project_root: Path,
    rust_cli: Path,
    configuration: str | None,
    search_directories: list[Path],
    windows_prefix_mappings: list[tuple[str, Path]],
    follow_suppressed: bool,
    profile: str,
    expected_paths: list[str] | None,
) -> tuple[dict[str, Any], bool]:
    keyword_options = {
        "project_root": project_root,
        "configuration": configuration,
        "search_directories": search_directories,
        "windows_prefix_mappings": windows_prefix_mappings,
        "follow_suppressed": follow_suppressed,
        "profile": profile,
    }
    first = sldkit.scan_project(root_document, **keyword_options).to_dict()
    second = sldkit.scan_project(root_document, **keyword_options).to_dict()
    native = json.loads(
        _core.scan_project_json(
            str(root_document),
            str(project_root),
            configuration,
            [str(path) for path in search_directories],
            [(source, str(target)) for source, target in windows_prefix_mappings],
            follow_suppressed,
            profile,
        )
    )
    rust = rust_result(
        rust_cli,
        root_document,
        project_root,
        configuration,
        search_directories,
        windows_prefix_mappings,
        follow_suppressed,
        profile,
    )
    deterministic = canonical(first) == canonical(second)
    native_parity = canonical(first) == canonical(native)
    rust_parity = canonical(first) == canonical(rust)
    report = first["compatibility_report"]
    report_path_free = not contains_path_fields(report)
    closure = (
        None if expected_paths is None else closure_difference(first, expected_paths)
    )
    passed = (
        first["status"] != "rejected"
        and deterministic
        and native_parity
        and rust_parity
        and report_path_free
        and (closure is None or closure["status"] == "match")
    )
    record = {
        "schema_version": 1,
        "passed": passed,
        "root_name": root_document.name,
        "status": first["status"],
        "graph_json_sha256": hashlib.sha256(canonical(first)).hexdigest(),
        "deterministic": deterministic,
        "python_native_json_parity": native_parity,
        "rust_cli_json_parity": rust_parity,
        "compatibility_report_path_free": report_path_free,
        "compatibility_report": report,
        "closure_difference": closure,
    }
    return record, passed


def load_expected_closure(path: Path | None) -> list[str] | None:
    if path is None:
        return None
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict) or value.get("schema_version") != 1:
        raise ValueError("expected closure must be a schema_version 1 JSON object")
    paths = value.get("paths")
    if not isinstance(paths, list) or not all(isinstance(item, str) for item in paths):
        raise ValueError("expected closure paths must be a JSON string array")
    return paths


def prefix_mapping(value: str) -> tuple[str, Path]:
    source, separator, target = value.partition("=")
    if not separator or not source or not target:
        raise argparse.ArgumentTypeError("expected non-empty SOURCE=TARGET")
    return source, Path(target)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root_document", type=Path)
    parser.add_argument("--project-root", required=True, type=Path)
    parser.add_argument("--rust-cli", required=True, type=Path)
    parser.add_argument("--configuration")
    parser.add_argument("--search-dir", action="append", default=[], type=Path)
    parser.add_argument(
        "--windows-prefix-map", action="append", default=[], type=prefix_mapping
    )
    parser.add_argument("--follow-suppressed", action="store_true")
    parser.add_argument("--expected-closure", type=Path)
    parser.add_argument("--profile", choices=("desktop", "service"), default="service")
    args = parser.parse_args()
    record, passed = validate(
        args.root_document,
        args.project_root,
        args.rust_cli,
        args.configuration,
        args.search_dir,
        args.windows_prefix_map,
        args.follow_suppressed,
        args.profile,
        load_expected_closure(args.expected_closure),
    )
    print(json.dumps(record, ensure_ascii=False, indent=2, sort_keys=True))
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
