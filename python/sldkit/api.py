from __future__ import annotations

import json
import os
from collections.abc import Iterable, Mapping
from os import PathLike
from typing import Any

from . import _core
from .errors import ParseError
from .model import (
    ExtractionMode,
    ExtractionResult,
    InventoryResult,
    LimitProfile,
    ParseResult,
    ParseStatus,
    ProbeResult,
    ProjectScanResult,
    StreamExtraction,
)

BytesLike = bytes | bytearray | memoryview
PathType = str | PathLike[str]
WindowsPrefixMappings = (
    Mapping[str, PathType] | Iterable[tuple[str, PathType]]
)


def probe_bytes(
    data: BytesLike,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> ProbeResult:
    """Classify a bounded byte input by content without semantic parsing."""
    payload = _load_json(_core.probe_bytes_json(bytes(data), _profile_name(profile)))
    return ProbeResult.from_dict(payload)


def probe_file(
    path: PathType,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> ProbeResult:
    """Read a file under a resource limit and classify its envelope."""
    payload = _load_json(
        _core.probe_file_json(os.fsdecode(os.fspath(path)), _profile_name(profile))
    )
    return ProbeResult.from_dict(payload)


def inspect_bytes(
    data: BytesLike,
    *,
    filename: str | None = None,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> InventoryResult:
    """Return a bounded, deterministic inventory without document semantics."""
    payload = _load_json(
        _core.inspect_bytes_json(bytes(data), filename, _profile_name(profile))
    )
    return InventoryResult.from_dict(payload)


def inspect_file(
    path: PathType,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> InventoryResult:
    """Read a file under a resource limit and inventory its outer container."""
    payload = _load_json(
        _core.inspect_file_json(os.fsdecode(os.fspath(path)), _profile_name(profile))
    )
    return InventoryResult.from_dict(payload)


def extract_bytes(
    data: BytesLike,
    entry_id: str,
    *,
    mode: str | ExtractionMode = ExtractionMode.DECODED,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> StreamExtraction:
    """Extract one validated entry representation without filesystem expansion."""
    metadata, payload = _core.extract_bytes_result(
        bytes(data), entry_id, _mode_name(mode), _profile_name(profile)
    )
    return StreamExtraction(
        result=ExtractionResult.from_dict(_load_json(metadata)), data=payload
    )


def extract_file(
    path: PathType,
    entry_id: str,
    *,
    mode: str | ExtractionMode = ExtractionMode.DECODED,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> StreamExtraction:
    """Extract one validated entry from a bounded source path."""
    metadata, payload = _core.extract_file_result(
        os.fsdecode(os.fspath(path)),
        entry_id,
        _mode_name(mode),
        _profile_name(profile),
    )
    return StreamExtraction(
        result=ExtractionResult.from_dict(_load_json(metadata)), data=payload
    )


def parse_bytes(
    data: BytesLike,
    *,
    filename: str | None = None,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
) -> ParseResult:
    """Parse supported facts while retaining partial or malformed results."""
    payload = _load_json(
        _core.parse_bytes_json(bytes(data), filename, _profile_name(profile))
    )
    return _apply_strict(ParseResult.from_dict(payload), strict)


def parse_file(
    path: PathType,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
) -> ParseResult:
    """Parse a path and retain its exact source identity and diagnostics."""
    payload = _load_json(
        _core.parse_file_json(os.fsdecode(os.fspath(path)), _profile_name(profile))
    )
    return _apply_strict(ParseResult.from_dict(payload), strict)


def scan_project(
    path: PathType,
    *,
    project_root: PathType | None = None,
    configuration: str | None = None,
    search_directories: Iterable[PathType] = (),
    windows_prefix_mappings: WindowsPrefixMappings | None = None,
    follow_suppressed: bool = False,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> ProjectScanResult:
    """Resolve a bounded local document graph without changing stored paths."""
    mappings = (
        ()
        if windows_prefix_mappings is None
        else (
            windows_prefix_mappings.items()
            if isinstance(windows_prefix_mappings, Mapping)
            else windows_prefix_mappings
        )
    )
    payload = _load_json(
        _core.scan_project_json(
            os.fsdecode(os.fspath(path)),
            (
                None
                if project_root is None
                else os.fsdecode(os.fspath(project_root))
            ),
            configuration,
            [os.fsdecode(os.fspath(item)) for item in search_directories],
            [
                (str(prefix), os.fsdecode(os.fspath(target)))
                for prefix, target in mappings
            ],
            follow_suppressed,
            _profile_name(profile),
        )
    )
    return ProjectScanResult.from_dict(payload)


def _load_json(value: str) -> Mapping[str, Any]:
    decoded = json.loads(value)
    if not isinstance(decoded, dict):
        raise RuntimeError("native parser returned a non-object JSON result")
    return decoded


def _profile_name(profile: str | LimitProfile) -> str:
    return profile.value if isinstance(profile, LimitProfile) else profile


def _mode_name(mode: str | ExtractionMode) -> str:
    return mode.value if isinstance(mode, ExtractionMode) else mode


def _apply_strict(result: ParseResult, strict: bool) -> ParseResult:
    if strict and result.status is not ParseStatus.PARSED:
        raise ParseError(result)
    return result
