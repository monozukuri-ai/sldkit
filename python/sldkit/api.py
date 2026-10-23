from __future__ import annotations

import json
import os
from collections.abc import Iterable, Mapping
from os import PathLike
from typing import Any

from . import _core
from .errors import (
    DisplayMeshError,
    DrawingStructureError,
    GeometryError,
    GeometryEvaluationError,
    ParseError,
)
from .model import (
    BillOfMaterials,
    BinaryResource,
    BomOptions,
    DisplayMeshResult,
    DisplayMeshStatus,
    DrawingStructureResult,
    DrawingStructureStatus,
    ExtractionMode,
    ExtractionResult,
    GeometryDocument,
    GeometryEvaluationOptions,
    GeometryEvaluationResult,
    GeometryEvaluationStatus,
    GeometryResult,
    GeometryStatus,
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
WindowsPrefixMappings = Mapping[str, PathType] | Iterable[tuple[str, PathType]]


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


def extract_resource_bytes(
    data: BytesLike,
    resource: BinaryResource,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> StreamExtraction:
    """Extract and revalidate the exact bytes described by a binary resource."""
    metadata, payload = _core.extract_resource_bytes_result(
        bytes(data),
        json.dumps(resource.to_dict(), separators=(",", ":"), sort_keys=True),
        _profile_name(profile),
    )
    return StreamExtraction(
        result=ExtractionResult.from_dict(_load_json(metadata)), data=payload
    )


def extract_resource_file(
    path: PathType,
    resource: BinaryResource,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> StreamExtraction:
    """Read a bounded file and extract one exact binary resource from it."""
    metadata, payload = _core.extract_resource_file_result(
        os.fsdecode(os.fspath(path)),
        json.dumps(resource.to_dict(), separators=(",", ":"), sort_keys=True),
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


def decode_geometry_bytes(
    data: BytesLike,
    *,
    filename: str | None = None,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
) -> GeometryResult:
    """Decode modern part geometry into the source-oriented geometry model."""
    payload = _load_json(
        _core.decode_geometry_bytes_json(bytes(data), filename, _profile_name(profile))
    )
    return _apply_geometry_strict(GeometryResult.from_dict(payload), strict)


def decode_geometry_file(
    path: PathType,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
) -> GeometryResult:
    """Decode modern part geometry while retaining provenance and loss details."""
    payload = _load_json(
        _core.decode_geometry_file_json(
            os.fsdecode(os.fspath(path)), _profile_name(profile)
        )
    )
    return _apply_geometry_strict(GeometryResult.from_dict(payload), strict)


def decode_display_mesh_bytes(
    data: BytesLike,
    *,
    filename: str | None = None,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
) -> DisplayMeshResult:
    """Decode saved display meshes without running the B-Rep decoder.

    The result carries packed little-endian buffers beside its JSON contract;
    see ``DisplayMeshResult``.
    """
    payload, positions, normals, indices, edges = _core.decode_display_mesh_bytes(
        bytes(data), filename, _profile_name(profile)
    )
    return _apply_display_strict(
        DisplayMeshResult.from_dict(
            _load_json(payload),
            positions=positions,
            normals=normals,
            indices=indices,
            edges=edges,
        ),
        strict,
    )


def decode_display_mesh_file(
    path: PathType,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
) -> DisplayMeshResult:
    """Decode a file's saved display meshes without running the B-Rep decoder."""
    payload, positions, normals, indices, edges = _core.decode_display_mesh_file(
        os.fsdecode(os.fspath(path)), _profile_name(profile)
    )
    return _apply_display_strict(
        DisplayMeshResult.from_dict(
            _load_json(payload),
            positions=positions,
            normals=normals,
            indices=indices,
            edges=edges,
        ),
        strict,
    )


def evaluate_geometry_bytes(
    data: BytesLike,
    *,
    filename: str | None = None,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
    options: GeometryEvaluationOptions | None = None,
    **option_overrides: Any,
) -> GeometryEvaluationResult:
    """Decode modern Part geometry and evaluate edges, bounds, and meshes.

    Options can be passed as a ``GeometryEvaluationOptions`` or as keyword
    overrides such as ``chord_tolerance_mm=0.05`` or ``configuration="Default"``.
    """
    payload = _load_json(
        _core.evaluate_geometry_bytes_json(
            bytes(data),
            filename,
            _profile_name(profile),
            _evaluation_options_json(options, option_overrides),
        )
    )
    return _apply_evaluation_strict(GeometryEvaluationResult.from_dict(payload), strict)


def evaluate_geometry_file(
    path: PathType,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
    options: GeometryEvaluationOptions | None = None,
    **option_overrides: Any,
) -> GeometryEvaluationResult:
    """Read a path, decode its modern Part geometry, and evaluate it."""
    payload = _load_json(
        _core.evaluate_geometry_file_json(
            os.fsdecode(os.fspath(path)),
            _profile_name(profile),
            _evaluation_options_json(options, option_overrides),
        )
    )
    return _apply_evaluation_strict(GeometryEvaluationResult.from_dict(payload), strict)


def evaluate_geometry(
    geometry: GeometryResult | GeometryDocument,
    *,
    strict: bool = False,
    options: GeometryEvaluationOptions | None = None,
    **option_overrides: Any,
) -> GeometryEvaluationResult:
    """Evaluate an already decoded geometry result or document.

    A ``GeometryResult`` without a geometry document yields ``UNAVAILABLE``.
    """
    document = geometry.geometry if isinstance(geometry, GeometryResult) else geometry
    if document is None:
        result = GeometryEvaluationResult(
            status=GeometryEvaluationStatus.UNAVAILABLE,
            evaluation=None,
            diagnostics=(),
        )
        return _apply_evaluation_strict(result, strict)
    payload = _load_json(
        _core.evaluate_geometry_json(
            json.dumps(document.to_dict(), separators=(",", ":")),
            _evaluation_options_json(options, option_overrides),
        )
    )
    return _apply_evaluation_strict(GeometryEvaluationResult.from_dict(payload), strict)


def _evaluation_options_json(
    options: GeometryEvaluationOptions | None, overrides: Mapping[str, Any]
) -> str:
    base = GeometryEvaluationOptions() if options is None else options
    unknown = set(overrides) - set(GeometryEvaluationOptions.__dataclass_fields__)
    if unknown:
        raise TypeError(f"unknown evaluation options: {sorted(unknown)}")
    merged = {**base.to_dict(), **overrides}
    return json.dumps(merged, separators=(",", ":"), sort_keys=True)


def decode_drawing_structure_bytes(
    data: BytesLike,
    *,
    filename: str | None = None,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
) -> DrawingStructureResult:
    """Inventory source-native records from a modern Drawing."""
    payload = _load_json(
        _core.decode_drawing_structure_bytes_json(
            bytes(data), filename, _profile_name(profile)
        )
    )
    return _apply_drawing_structure_strict(
        DrawingStructureResult.from_dict(payload), strict
    )


def decode_drawing_structure_file(
    path: PathType,
    *,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
    strict: bool = False,
) -> DrawingStructureResult:
    """Inventory Drawing records while retaining exact source ranges and carriers."""
    payload = _load_json(
        _core.decode_drawing_structure_file_json(
            os.fsdecode(os.fspath(path)), _profile_name(profile)
        )
    )
    return _apply_drawing_structure_strict(
        DrawingStructureResult.from_dict(payload), strict
    )


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
            (None if project_root is None else os.fsdecode(os.fspath(project_root))),
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


def index_project(
    project_root: PathType,
    *,
    search_directories: Iterable[PathType] = (),
    windows_prefix_mappings: WindowsPrefixMappings | None = None,
    follow_suppressed: bool = False,
    profile: str | LimitProfile = LimitProfile.DESKTOP,
) -> ProjectScanResult:
    """Parse every document under a root and resolve all references.

    Each indexed document is a root node, so ``ProjectScanResult.edges_to`` and
    ``referencing_nodes`` answer which Drawings or Assemblies use a document.
    """
    payload = _load_json(
        _core.index_project_json(
            os.fsdecode(os.fspath(project_root)),
            [os.fsdecode(os.fspath(item)) for item in search_directories],
            _prefix_mapping_pairs(windows_prefix_mappings),
            follow_suppressed,
            _profile_name(profile),
        )
    )
    return ProjectScanResult.from_dict(payload)


def build_bom(
    graph: ProjectScanResult,
    *,
    root_configuration: str | None = None,
    include_suppressed: bool = False,
    include_excluded: bool = False,
    max_occurrences: int = 1_000_000,
) -> BillOfMaterials:
    """Expand a scanned graph's root occurrence tree into aggregated quantities.

    The root configuration must be unambiguous: pass its exact name, or scan
    the project with a selected configuration.
    """
    options = BomOptions(
        root_configuration=root_configuration,
        include_suppressed=include_suppressed,
        include_excluded=include_excluded,
        max_occurrences=max_occurrences,
    )
    payload = _load_json(
        _core.build_bom_json(
            json.dumps(graph.to_dict(), separators=(",", ":"), sort_keys=True),
            json.dumps(options.to_dict(), separators=(",", ":"), sort_keys=True),
        )
    )
    return BillOfMaterials.from_dict(payload)


def _prefix_mapping_pairs(
    mappings: WindowsPrefixMappings | None,
) -> list[tuple[str, str]]:
    if mappings is None:
        return []
    items = mappings.items() if isinstance(mappings, Mapping) else mappings
    return [(str(prefix), os.fsdecode(os.fspath(target))) for prefix, target in items]


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


def _apply_geometry_strict(result: GeometryResult, strict: bool) -> GeometryResult:
    if strict and result.status is not GeometryStatus.DECODED:
        raise GeometryError(result)
    return result


def _apply_display_strict(result: DisplayMeshResult, strict: bool) -> DisplayMeshResult:
    if strict and result.status is not DisplayMeshStatus.DECODED:
        raise DisplayMeshError(result)
    return result


def _apply_evaluation_strict(
    result: GeometryEvaluationResult, strict: bool
) -> GeometryEvaluationResult:
    if strict and result.status is not GeometryEvaluationStatus.EVALUATED:
        raise GeometryEvaluationError(result)
    return result


def _apply_drawing_structure_strict(
    result: DrawingStructureResult, strict: bool
) -> DrawingStructureResult:
    if strict and result.status is not DrawingStructureStatus.INVENTORIED:
        raise DrawingStructureError(result)
    return result
