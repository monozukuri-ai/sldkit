from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Generic, TypeVar

T = TypeVar("T")


class _StringEnum(str, Enum):
    def __str__(self) -> str:
        return self.value


class Envelope(_StringEnum):
    MODERN_CHUNK = "modern_chunk"
    OLE2_CFB = "ole2_cfb"
    ZIP_OPC = "zip_opc"
    UNKNOWN = "unknown"


class DocumentKind(_StringEnum):
    PART = "part"
    ASSEMBLY = "assembly"
    DRAWING = "drawing"
    UNKNOWN = "unknown"


class PropertyKind(_StringEnum):
    CUSTOM = "custom"
    CORE = "core"
    SYSTEM = "system"


class PropertyValueState(_StringEnum):
    PRESENT = "present"
    EMPTY = "empty"
    MISSING = "missing"
    UNSUPPORTED_TYPE = "unsupported_type"


class PropertyScope(_StringEnum):
    GLOBAL = "global"
    CONFIGURATION = "configuration"


class BinaryResourceKind(_StringEnum):
    PREVIEW_PNG = "preview_png"
    PREVIEW_DIB = "preview_dib"


class ReferenceKind(_StringEnum):
    ASSEMBLY_COMPONENT = "assembly_component"
    DRAWING_VIEW = "drawing_view"
    EXTERNAL_FEATURE = "external_feature"
    UNKNOWN = "unknown"


class RecordOffsetBasis(_StringEnum):
    SOURCE_FILE = "source_file"
    DECODED_STREAM = "decoded_stream"


class ValueOrigin(_StringEnum):
    SOURCE = "source"
    DERIVED = "derived"
    INFERRED = "inferred"
    HINT = "hint"
    PRESERVED = "preserved"


class SourceInputKind(_StringEnum):
    PATH = "path"
    BYTES = "bytes"


class DiagnosticSeverity(_StringEnum):
    INFO = "info"
    WARNING = "warning"
    ERROR = "error"


class DiagnosticKind(_StringEnum):
    FATAL = "fatal"
    UNSUPPORTED = "unsupported"
    MALFORMED = "malformed"
    UNRESOLVED = "unresolved"
    INFERRED = "inferred"
    PRESERVED = "preserved"


class ProbeStatus(_StringEnum):
    RECOGNIZED = "recognized"
    UNRECOGNIZED = "unrecognized"
    MALFORMED = "malformed"
    REJECTED = "rejected"


class ProbeConfidence(_StringEnum):
    HIGH = "high"
    MEDIUM = "medium"
    NONE = "none"


class ParseStatus(_StringEnum):
    PARSED = "parsed"
    PARTIAL = "partial"
    UNSUPPORTED = "unsupported"
    MALFORMED = "malformed"
    REJECTED = "rejected"


class ProjectScanStatus(_StringEnum):
    COMPLETE = "complete"
    PARTIAL = "partial"
    REJECTED = "rejected"


class ReferenceResolutionStatus(_StringEnum):
    RESOLVED = "resolved"
    MISSING = "missing"
    AMBIGUOUS = "ambiguous"
    NO_STORED_PATH = "no_stored_path"


class ReferenceResolutionBasis(_StringEnum):
    EMBEDDED_VIRTUAL_COMPONENT = "embedded_virtual_component"
    DOCUMENT_RELATIVE = "document_relative"
    DOCUMENT_BASENAME = "document_basename"
    PROJECT_ROOT_RELATIVE = "project_root_relative"
    PROJECT_ROOT_BASENAME = "project_root_basename"
    WINDOWS_PREFIX_MAPPING = "windows_prefix_mapping"
    SEARCH_DIRECTORY_RELATIVE = "search_directory_relative"
    SEARCH_DIRECTORY_BASENAME = "search_directory_basename"
    HOST_ABSOLUTE = "host_absolute"


class EmbeddedDocumentRole(_StringEnum):
    VIRTUAL_COMPONENT = "virtual_component"


class BomStatus(_StringEnum):
    COMPLETE = "complete"
    PARTIAL = "partial"
    REJECTED = "rejected"


class BomChildrenState(_StringEnum):
    EXPANDED = "expanded"
    LEAF = "leaf"
    SUPPRESSED = "suppressed"
    UNRESOLVED = "unresolved"
    CONFIGURATION_UNRESOLVED = "configuration_unresolved"
    NOT_TRAVERSED = "not_traversed"
    LIMIT_REACHED = "limit_reached"


class ReferenceTraversalStatus(_StringEnum):
    FOLLOWED = "followed"
    REUSED = "reused"
    SUPPRESSED = "suppressed"
    CYCLE = "cycle"
    DEPTH_LIMITED = "depth_limited"
    UNRESOLVED = "unresolved"


class InventoryStatus(_StringEnum):
    COMPLETE = "complete"
    PARTIAL = "partial"
    UNSUPPORTED = "unsupported"
    MALFORMED = "malformed"
    REJECTED = "rejected"


class InventoryEntryKind(_StringEnum):
    STREAM = "stream"
    STORAGE = "storage"
    BLOCK = "block"
    CACHE_CELL = "cache_cell"
    DIRECTORY_ENTRY = "directory_entry"
    ZIP_ENTRY = "zip_entry"


class InventoryEntryState(_StringEnum):
    DECODED = "decoded"
    STORED = "stored"
    METADATA_ONLY = "metadata_only"
    UNSUPPORTED = "unsupported"
    MALFORMED = "malformed"


class CompressionMethod(_StringEnum):
    NONE = "none"
    DEFLATE_RAW = "deflate_raw"
    ZLIB = "zlib"
    ZIP_STORED = "zip_stored"
    ZIP_DEFLATE = "zip_deflate"
    UNSUPPORTED = "unsupported"


class ChecksumStatus(_StringEnum):
    VERIFIED = "verified"
    MISMATCH = "mismatch"
    NOT_PRESENT = "not_present"
    NOT_CHECKED = "not_checked"


class ExtractionMode(_StringEnum):
    STORED = "stored"
    DECODED = "decoded"


class ExtractionStatus(_StringEnum):
    EXTRACTED = "extracted"
    NOT_FOUND = "not_found"
    UNAVAILABLE = "unavailable"
    MALFORMED = "malformed"
    REJECTED = "rejected"


class LimitProfile(_StringEnum):
    DESKTOP = "desktop"
    SERVICE = "service"


class DrawingStructureStatus(_StringEnum):
    INVENTORIED = "inventoried"
    PARTIAL = "partial"
    UNSUPPORTED = "unsupported"
    MALFORMED = "malformed"
    REJECTED = "rejected"


class DrawingRecordClass(_StringEnum):
    ROOT = "root"
    ATTRIBUTE = "attribute"
    FEATURE = "feature"
    LAYER = "layer"
    NOTE = "note"
    REFERENCE = "reference"
    SHEET = "sheet"
    VIEW = "view"
    SKETCH = "sketch"
    FIELD = "field"
    OTHER = "other"


class DrawingCarrierRole(_StringEnum):
    DEFINITION_CANDIDATE = "definition_candidate"
    DISPLAY_LISTS_CANDIDATE = "display_lists_candidate"
    VB_LISTS_CANDIDATE = "vb_lists_candidate"


class DrawingBytePartitionStatus(_StringEnum):
    COMPLETE = "complete"
    INCOMPLETE = "incomplete"


class GeometryStatus(_StringEnum):
    DECODED = "decoded"
    PARTIAL = "partial"
    UNSUPPORTED = "unsupported"
    MALFORMED = "malformed"
    REJECTED = "rejected"


class GeometryStreamRole(_StringEnum):
    PARASOLID_PARTITION = "parasolid_partition"
    PARASOLID_DELTAS = "parasolid_deltas"
    TESSELLATION = "tessellation"


class GeometryStreamSelection(_StringEnum):
    ACTIVE = "active"
    ALTERNATE = "alternate"
    SUPPORTING = "supporting"
    CANDIDATE = "candidate"


class GeometryExactness(_StringEnum):
    BYTE_EXACT = "byte_exact"
    DERIVED = "derived"
    INFERRED = "inferred"
    UNKNOWN = "unknown"


class GeometryByteClassification(_StringEnum):
    TYPED = "typed"
    UNINTERPRETED = "uninterpreted"


class GeometryBytePartitionStatus(_StringEnum):
    COMPLETE = "complete"
    INCOMPLETE = "incomplete"


class GeometryByteStorage(_StringEnum):
    DIRECT = "direct"
    WRAPPED_ZLIB = "wrapped_zlib"


class GeometryByteOffsetBasis(_StringEnum):
    PARASOLID_BODY = "parasolid_body"


class GeometryEvaluationStatus(_StringEnum):
    EVALUATED = "evaluated"
    PARTIAL = "partial"
    UNAVAILABLE = "unavailable"
    REJECTED = "rejected"


class EdgeIntervalSource(_StringEnum):
    SOURCE = "source"
    DERIVED = "derived"
    UNAVAILABLE = "unavailable"


class EdgeSampleStatus(_StringEnum):
    SAMPLED = "sampled"
    UNSAMPLED = "unsampled"


class EdgeSmoothness(_StringEnum):
    SHARP = "sharp"
    TANGENT = "tangent"
    BOUNDARY = "boundary"
    UNKNOWN = "unknown"


class BoundingBoxMethod(_StringEnum):
    EDGE_SAMPLES = "edge_samples"
    TESSELLATION = "tessellation"
    EDGE_SAMPLES_AND_TESSELLATION = "edge_samples_and_tessellation"


class GeometryCarrierDomain(_StringEnum):
    SURFACE = "surface"
    CURVE = "curve"
    PCURVE = "pcurve"


class GeometryConstructionDomain(_StringEnum):
    SURFACE = "surface"
    CURVE = "curve"


@dataclass(frozen=True, slots=True)
class Diagnostic:
    code: str
    severity: DiagnosticSeverity
    kind: DiagnosticKind
    message: str
    offset: int | None = None
    stream_path: str | None = None
    details: Mapping[str, str] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> Diagnostic:
        return cls(
            code=str(value["code"]),
            severity=DiagnosticSeverity(value["severity"]),
            kind=DiagnosticKind(value["kind"]),
            message=str(value["message"]),
            offset=_optional_int(value.get("offset")),
            stream_path=_optional_str(value.get("stream_path")),
            details={
                str(key): str(item) for key, item in value.get("details", {}).items()
            },
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "code": self.code,
            "severity": self.severity.value,
            "kind": self.kind.value,
            "message": self.message,
            "offset": self.offset,
            "stream_path": self.stream_path,
        }
        if self.details:
            result["details"] = dict(self.details)
        return result


@dataclass(frozen=True, slots=True)
class CoverageReport:
    total_bytes: int
    inspected_bytes: int
    decoded_bytes: int
    uninterpreted_bytes: int
    streams_total: int
    streams_decoded: int

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> CoverageReport:
        return cls(**{name: int(value[name]) for name in cls.__dataclass_fields__})

    def to_dict(self) -> dict[str, int]:
        return {name: int(getattr(self, name)) for name in self.__dataclass_fields__}


@dataclass(frozen=True, slots=True)
class ByteRange:
    offset: int
    length: int

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ByteRange:
        return cls(offset=int(value["offset"]), length=int(value["length"]))

    def to_dict(self) -> dict[str, int]:
        return {"offset": self.offset, "length": self.length}


@dataclass(frozen=True, slots=True)
class InventoryEntry:
    id: str
    path: str | None
    kind: InventoryEntryKind
    state: InventoryEntryState
    source_range: ByteRange | None
    payload_range: ByteRange | None
    stored_size: int
    decoded_size: int | None
    compression: CompressionMethod
    checksum: ChecksumStatus
    expected_crc32: int | None
    stored_sha256: str | None
    decoded_sha256: str | None
    attributes: Mapping[str, str] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> InventoryEntry:
        source_range = value.get("source_range")
        payload_range = value.get("payload_range")
        return cls(
            id=str(value["id"]),
            path=_optional_str(value.get("path")),
            kind=InventoryEntryKind(value["kind"]),
            state=InventoryEntryState(value["state"]),
            source_range=(
                None if source_range is None else ByteRange.from_dict(source_range)
            ),
            payload_range=(
                None if payload_range is None else ByteRange.from_dict(payload_range)
            ),
            stored_size=int(value["stored_size"]),
            decoded_size=_optional_int(value.get("decoded_size")),
            compression=CompressionMethod(value["compression"]),
            checksum=ChecksumStatus(value["checksum"]),
            expected_crc32=_optional_int(value.get("expected_crc32")),
            stored_sha256=_optional_str(value.get("stored_sha256")),
            decoded_sha256=_optional_str(value.get("decoded_sha256")),
            attributes={
                str(key): str(item) for key, item in value.get("attributes", {}).items()
            },
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "path": self.path,
            "kind": self.kind.value,
            "state": self.state.value,
            "source_range": (
                None if self.source_range is None else self.source_range.to_dict()
            ),
            "payload_range": (
                None if self.payload_range is None else self.payload_range.to_dict()
            ),
            "stored_size": self.stored_size,
            "decoded_size": self.decoded_size,
            "compression": self.compression.value,
            "checksum": self.checksum.value,
            "expected_crc32": self.expected_crc32,
            "stored_sha256": self.stored_sha256,
            "decoded_sha256": self.decoded_sha256,
        }
        if self.attributes:
            result["attributes"] = dict(self.attributes)
        return result


@dataclass(frozen=True, slots=True)
class ContainerInventory:
    envelope: Envelope
    format_version: int | None
    entries: tuple[InventoryEntry, ...]
    attributes: Mapping[str, str] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ContainerInventory:
        return cls(
            envelope=Envelope(value["envelope"]),
            format_version=_optional_int(value.get("format_version")),
            entries=tuple(
                InventoryEntry.from_dict(item) for item in value.get("entries", [])
            ),
            attributes={
                str(key): str(item) for key, item in value.get("attributes", {}).items()
            },
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "envelope": self.envelope.value,
            "format_version": self.format_version,
            "entries": [entry.to_dict() for entry in self.entries],
        }
        if self.attributes:
            result["attributes"] = dict(self.attributes)
        return result


@dataclass(frozen=True, slots=True)
class InventoryResult:
    status: InventoryStatus
    inventory: ContainerInventory | None
    diagnostics: tuple[Diagnostic, ...]
    coverage: CoverageReport
    uninterpreted_ranges: tuple[ByteRange, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> InventoryResult:
        inventory = value.get("inventory")
        return cls(
            status=InventoryStatus(value["status"]),
            inventory=(
                None if inventory is None else ContainerInventory.from_dict(inventory)
            ),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
            coverage=CoverageReport.from_dict(value["coverage"]),
            uninterpreted_ranges=tuple(
                ByteRange.from_dict(item)
                for item in value.get("uninterpreted_ranges", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "inventory": (None if self.inventory is None else self.inventory.to_dict()),
            "diagnostics": [item.to_dict() for item in self.diagnostics],
            "coverage": self.coverage.to_dict(),
            "uninterpreted_ranges": [
                item.to_dict() for item in self.uninterpreted_ranges
            ],
        }


@dataclass(frozen=True, slots=True)
class ExtractionResult:
    status: ExtractionStatus
    mode: ExtractionMode
    entry: InventoryEntry | None
    byte_len: int | None
    sha256: str | None
    diagnostics: tuple[Diagnostic, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ExtractionResult:
        entry = value.get("entry")
        return cls(
            status=ExtractionStatus(value["status"]),
            mode=ExtractionMode(value["mode"]),
            entry=None if entry is None else InventoryEntry.from_dict(entry),
            byte_len=_optional_int(value.get("byte_len")),
            sha256=_optional_str(value.get("sha256")),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "mode": self.mode.value,
            "entry": None if self.entry is None else self.entry.to_dict(),
            "byte_len": self.byte_len,
            "sha256": self.sha256,
            "diagnostics": [item.to_dict() for item in self.diagnostics],
        }


@dataclass(frozen=True, slots=True)
class StreamExtraction:
    result: ExtractionResult
    data: bytes | None


@dataclass(frozen=True, slots=True)
class ProbeEvidence:
    code: str
    offset: int
    length: int
    description: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ProbeEvidence:
        return cls(
            code=str(value["code"]),
            offset=int(value["offset"]),
            length=int(value["length"]),
            description=str(value["description"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "code": self.code,
            "offset": self.offset,
            "length": self.length,
            "description": self.description,
        }


@dataclass(frozen=True, slots=True)
class ProbeResult:
    status: ProbeStatus
    envelope: Envelope
    confidence: ProbeConfidence
    evidence: tuple[ProbeEvidence, ...]
    diagnostics: tuple[Diagnostic, ...]
    coverage: CoverageReport

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ProbeResult:
        return cls(
            status=ProbeStatus(value["status"]),
            envelope=Envelope(value["envelope"]),
            confidence=ProbeConfidence(value["confidence"]),
            evidence=tuple(
                ProbeEvidence.from_dict(item) for item in value.get("evidence", [])
            ),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
            coverage=CoverageReport.from_dict(value["coverage"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "envelope": self.envelope.value,
            "confidence": self.confidence.value,
            "evidence": [item.to_dict() for item in self.evidence],
            "diagnostics": [item.to_dict() for item in self.diagnostics],
            "coverage": self.coverage.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class SourcedValue(Generic[T]):
    value: T
    origin: ValueOrigin
    evidence: tuple[str, ...] = ()

    @classmethod
    def from_dict(
        cls,
        value: Mapping[str, Any],
        converter: Callable[[Any], T],
    ) -> SourcedValue[T]:
        return cls(
            value=converter(value["value"]),
            origin=ValueOrigin(value["origin"]),
            evidence=tuple(str(item) for item in value.get("evidence", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        raw_value = self.value.value if isinstance(self.value, Enum) else self.value
        result = {
            "value": raw_value,
            "origin": self.origin.value,
        }
        if self.evidence:
            result["evidence"] = list(self.evidence)
        return result


@dataclass(frozen=True, slots=True)
class SourceInfo:
    input_kind: SourceInputKind
    label: str | None
    byte_len: int
    sha256: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> SourceInfo:
        return cls(
            input_kind=SourceInputKind(value["input_kind"]),
            label=_optional_str(value.get("label")),
            byte_len=int(value["byte_len"]),
            sha256=str(value["sha256"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "input_kind": self.input_kind.value,
            "label": self.label,
            "byte_len": self.byte_len,
            "sha256": self.sha256,
        }


@dataclass(frozen=True, slots=True)
class BinaryResource:
    kind: BinaryResourceKind
    entry_id: str
    stream_path: str
    decoded_offset: int
    byte_len: int
    sha256: str
    media_type: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> BinaryResource:
        return cls(
            kind=BinaryResourceKind(value["kind"]),
            entry_id=str(value["entry_id"]),
            stream_path=str(value["stream_path"]),
            decoded_offset=int(value["decoded_offset"]),
            byte_len=int(value["byte_len"]),
            sha256=str(value["sha256"]),
            media_type=str(value["media_type"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind.value,
            "entry_id": self.entry_id,
            "stream_path": self.stream_path,
            "decoded_offset": self.decoded_offset,
            "byte_len": self.byte_len,
            "sha256": self.sha256,
            "media_type": self.media_type,
        }


@dataclass(frozen=True, slots=True)
class MassProperties:
    raw_value: SourcedValue[str]
    center_of_gravity: tuple[str, str, str]
    volume: str
    surface_area: str
    mass: str
    moments_of_inertia: tuple[str, str, str]
    products_of_inertia: tuple[str, str, str]
    additional_values: tuple[str, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> MassProperties:
        return cls(
            raw_value=SourcedValue.from_dict(value["raw_value"], str),
            center_of_gravity=_string_triple(value["center_of_gravity"]),
            volume=str(value["volume"]),
            surface_area=str(value["surface_area"]),
            mass=str(value["mass"]),
            moments_of_inertia=_string_triple(value["moments_of_inertia"]),
            products_of_inertia=_string_triple(value["products_of_inertia"]),
            additional_values=tuple(
                str(item) for item in value.get("additional_values", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {
            "raw_value": self.raw_value.to_dict(),
            "center_of_gravity": list(self.center_of_gravity),
            "volume": self.volume,
            "surface_area": self.surface_area,
            "mass": self.mass,
            "moments_of_inertia": list(self.moments_of_inertia),
            "products_of_inertia": list(self.products_of_inertia),
        }
        if self.additional_values:
            result["additional_values"] = list(self.additional_values)
        return result


@dataclass(frozen=True, slots=True)
class AssemblyComponent:
    configuration_index: int
    instance_name: SourcedValue[str] | None
    stored_path: SourcedValue[str] | None
    document_kind: SourcedValue[DocumentKind] | None
    referenced_configuration: SourcedValue[str] | None
    component_reference: SourcedValue[str] | None
    is_suppressed: SourcedValue[bool] | None
    is_hidden: SourcedValue[bool] | None
    exclude_from_bom: SourcedValue[bool] | None
    source_model_ref: str | None
    raw_attributes: Mapping[str, str] = field(default_factory=dict)
    is_virtual: SourcedValue[bool] | None = None
    is_flexible: SourcedValue[bool] | None = None
    instance_number: SourcedValue[int] | None = None
    component_id: SourcedValue[int] | None = None
    raw_transform: SourcedValue[tuple[str, ...]] | None = None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> AssemblyComponent:
        return cls(
            configuration_index=int(value["configuration_index"]),
            instance_name=_optional_sourced(value.get("instance_name"), str),
            stored_path=_optional_sourced(value.get("stored_path"), str),
            document_kind=_optional_sourced(value.get("document_kind"), DocumentKind),
            referenced_configuration=_optional_sourced(
                value.get("referenced_configuration"), str
            ),
            component_reference=_optional_sourced(
                value.get("component_reference"), str
            ),
            is_suppressed=_optional_sourced(value.get("is_suppressed"), bool),
            is_hidden=_optional_sourced(value.get("is_hidden"), bool),
            exclude_from_bom=_optional_sourced(value.get("exclude_from_bom"), bool),
            source_model_ref=_optional_str(value.get("source_model_ref")),
            raw_attributes={
                str(key): str(item)
                for key, item in value.get("raw_attributes", {}).items()
            },
            is_virtual=_optional_sourced(value.get("is_virtual"), bool),
            is_flexible=_optional_sourced(value.get("is_flexible"), bool),
            instance_number=_optional_sourced(value.get("instance_number"), int),
            component_id=_optional_sourced(value.get("component_id"), int),
            raw_transform=_optional_sourced(value.get("raw_transform"), _string_tuple),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {
            "configuration_index": self.configuration_index,
            "instance_name": _sourced_dict(self.instance_name),
            "stored_path": _sourced_dict(self.stored_path),
            "document_kind": _sourced_dict(self.document_kind),
            "referenced_configuration": _sourced_dict(self.referenced_configuration),
            "component_reference": _sourced_dict(self.component_reference),
            "is_suppressed": _sourced_dict(self.is_suppressed),
            "is_hidden": _sourced_dict(self.is_hidden),
            "exclude_from_bom": _sourced_dict(self.exclude_from_bom),
            "source_model_ref": self.source_model_ref,
        }
        if self.raw_attributes:
            result["raw_attributes"] = dict(self.raw_attributes)
        for name in (
            "is_virtual",
            "is_flexible",
            "instance_number",
            "component_id",
        ):
            item = getattr(self, name)
            if item is not None:
                result[name] = item.to_dict()
        if self.raw_transform is not None:
            result["raw_transform"] = {
                **self.raw_transform.to_dict(),
                "value": list(self.raw_transform.value),
            }
        return result


@dataclass(frozen=True, slots=True)
class ModelBoundingBox:
    raw_value: SourcedValue[str]
    min: tuple[str, str, str]
    max: tuple[str, str, str]
    length_unit: SourcedValue[str]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ModelBoundingBox:
        return cls(
            raw_value=SourcedValue.from_dict(value["raw_value"], str),
            min=_string_triple(value["min"]),
            max=_string_triple(value["max"]),
            length_unit=SourcedValue.from_dict(value["length_unit"], str),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "raw_value": self.raw_value.to_dict(),
            "min": list(self.min),
            "max": list(self.max),
            "length_unit": self.length_unit.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class Configuration:
    index: SourcedValue[int]
    name: SourcedValue[str] | None
    alternate_names: tuple[SourcedValue[str], ...]
    parent_name: SourcedValue[str] | None
    parent_index: SourcedValue[int] | None
    preview: BinaryResource | None
    mass_properties: MassProperties | None
    components: tuple[AssemblyComponent, ...]
    material: SourcedValue[str] | None = None
    bounding_box: ModelBoundingBox | None = None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> Configuration:
        preview = value.get("preview")
        mass_properties = value.get("mass_properties")
        bounding_box = value.get("bounding_box")
        return cls(
            index=SourcedValue.from_dict(value["index"], int),
            name=_optional_sourced(value.get("name"), str),
            alternate_names=tuple(
                SourcedValue.from_dict(item, str)
                for item in value.get("alternate_names", [])
            ),
            parent_name=_optional_sourced(value.get("parent_name"), str),
            parent_index=_optional_sourced(value.get("parent_index"), int),
            preview=(None if preview is None else BinaryResource.from_dict(preview)),
            mass_properties=(
                None
                if mass_properties is None
                else MassProperties.from_dict(mass_properties)
            ),
            components=tuple(
                AssemblyComponent.from_dict(item)
                for item in value.get("components", [])
            ),
            material=_optional_sourced(value.get("material"), str),
            bounding_box=(
                None
                if bounding_box is None
                else ModelBoundingBox.from_dict(bounding_box)
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {
            "index": self.index.to_dict(),
            "name": _sourced_dict(self.name),
            "parent_name": _sourced_dict(self.parent_name),
            "parent_index": _sourced_dict(self.parent_index),
            "preview": None if self.preview is None else self.preview.to_dict(),
            "mass_properties": (
                None if self.mass_properties is None else self.mass_properties.to_dict()
            ),
        }
        if self.alternate_names:
            result["alternate_names"] = [
                item.to_dict() for item in self.alternate_names
            ]
        if self.material is not None:
            result["material"] = self.material.to_dict()
        if self.bounding_box is not None:
            result["bounding_box"] = self.bounding_box.to_dict()
        if self.components:
            result["components"] = [item.to_dict() for item in self.components]
        return result


@dataclass(frozen=True, slots=True)
class CustomProperty:
    name: SourcedValue[str]
    raw_value: SourcedValue[str] | None
    value_type: str | None
    value_state: PropertyValueState
    kind: PropertyKind
    scope: PropertyScope
    configuration: str | None
    configuration_index: int | None
    stream_path: str
    pid: int | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> CustomProperty:
        return cls(
            name=SourcedValue.from_dict(value["name"], str),
            raw_value=_optional_sourced(value.get("raw_value"), str),
            value_type=_optional_str(value.get("value_type")),
            value_state=PropertyValueState(value["value_state"]),
            kind=PropertyKind(value["kind"]),
            scope=PropertyScope(value["scope"]),
            configuration=_optional_str(value.get("configuration")),
            configuration_index=_optional_int(value.get("configuration_index")),
            stream_path=str(value["stream_path"]),
            pid=_optional_int(value.get("pid")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "name": self.name.to_dict(),
            "raw_value": _sourced_dict(self.raw_value),
            "value_type": self.value_type,
            "value_state": self.value_state.value,
            "kind": self.kind.value,
            "scope": self.scope.value,
            "configuration": self.configuration,
            "configuration_index": self.configuration_index,
            "stream_path": self.stream_path,
            "pid": self.pid,
        }


@dataclass(frozen=True, slots=True)
class DocumentReference:
    kind: ReferenceKind
    source_name: SourcedValue[str] | None
    stored_path: SourcedValue[str] | None
    resolved_path: str | None
    document_kind: SourcedValue[DocumentKind] | None
    configuration: SourcedValue[str] | None
    configuration_index: int | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DocumentReference:
        return cls(
            kind=ReferenceKind(value["kind"]),
            source_name=_optional_sourced(value.get("source_name"), str),
            stored_path=_optional_sourced(value.get("stored_path"), str),
            resolved_path=_optional_str(value.get("resolved_path")),
            document_kind=_optional_sourced(value.get("document_kind"), DocumentKind),
            configuration=_optional_sourced(value.get("configuration"), str),
            configuration_index=_optional_int(value.get("configuration_index")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "kind": self.kind.value,
            "source_name": _sourced_dict(self.source_name),
            "stored_path": _sourced_dict(self.stored_path),
            "resolved_path": self.resolved_path,
            "document_kind": _sourced_dict(self.document_kind),
            "configuration": _sourced_dict(self.configuration),
            "configuration_index": self.configuration_index,
        }


@dataclass(frozen=True, slots=True)
class DrawingView:
    source_id: str | None
    name: SourcedValue[str] | None
    referenced_document: SourcedValue[str] | None
    referenced_configuration: SourcedValue[str] | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingView:
        return cls(
            source_id=_optional_str(value.get("source_id")),
            name=_optional_sourced(value.get("name"), str),
            referenced_document=_optional_sourced(
                value.get("referenced_document"), str
            ),
            referenced_configuration=_optional_sourced(
                value.get("referenced_configuration"), str
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source_id": self.source_id,
            "name": _sourced_dict(self.name),
            "referenced_document": _sourced_dict(self.referenced_document),
            "referenced_configuration": _sourced_dict(self.referenced_configuration),
        }


@dataclass(frozen=True, slots=True)
class DrawingSheetFormatData:
    raw_value: SourcedValue[str]
    fields: tuple[str, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingSheetFormatData:
        return cls(
            raw_value=SourcedValue.from_dict(value["raw_value"], str),
            fields=tuple(str(item) for item in value.get("fields", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return {"raw_value": self.raw_value.to_dict(), "fields": list(self.fields)}


_SHEET_OPTIONAL_SOURCED = (
    "paper_size",
    "paper_size_name",
    "scale_numerator",
    "scale_denominator",
    "sheet_format_path",
    "custom_property_view",
    "width",
    "height",
)


@dataclass(frozen=True, slots=True)
class DrawingSheet:
    source_id: str | None
    name: SourcedValue[str] | None
    preview: BinaryResource | None
    views: tuple[DrawingView, ...]
    paper_size: SourcedValue[str] | None = None
    paper_size_code: SourcedValue[int] | None = None
    paper_size_name: SourcedValue[str] | None = None
    scale_numerator: SourcedValue[str] | None = None
    scale_denominator: SourcedValue[str] | None = None
    sheet_format_path: SourcedValue[str] | None = None
    custom_property_view: SourcedValue[str] | None = None
    width: SourcedValue[str] | None = None
    height: SourcedValue[str] | None = None
    format_data: DrawingSheetFormatData | None = None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingSheet:
        preview = value.get("preview")
        format_data = value.get("format_data")
        return cls(
            source_id=_optional_str(value.get("source_id")),
            name=_optional_sourced(value.get("name"), str),
            preview=(None if preview is None else BinaryResource.from_dict(preview)),
            views=tuple(DrawingView.from_dict(item) for item in value.get("views", [])),
            paper_size_code=_optional_sourced(value.get("paper_size_code"), int),
            format_data=(
                None
                if format_data is None
                else DrawingSheetFormatData.from_dict(format_data)
            ),
            **{
                name: _optional_sourced(value.get(name), str)
                for name in _SHEET_OPTIONAL_SOURCED
            },
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "source_id": self.source_id,
            "name": _sourced_dict(self.name),
            "preview": None if self.preview is None else self.preview.to_dict(),
            "views": [item.to_dict() for item in self.views],
        }
        for name in (*_SHEET_OPTIONAL_SOURCED, "paper_size_code"):
            item = getattr(self, name)
            if item is not None:
                result[name] = item.to_dict()
        if self.format_data is not None:
            result["format_data"] = self.format_data.to_dict()
        return result


@dataclass(frozen=True, slots=True)
class UnitSetting:
    property_name: str
    code: SourcedValue[int]
    name: SourcedValue[str] | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> UnitSetting:
        return cls(
            property_name=str(value["property_name"]),
            code=SourcedValue.from_dict(value["code"], int),
            name=_optional_sourced(value.get("name"), str),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "property_name": self.property_name,
            "code": self.code.to_dict(),
            "name": _sourced_dict(self.name),
        }


_UNIT_SETTING_FIELDS = (
    "unit_system",
    "length",
    "dual_length",
    "angle",
    "mass_property_length",
    "mass",
    "volume",
)
_UNIT_PLACES_FIELDS = (
    "length_decimal_places",
    "dual_length_decimal_places",
    "angle_decimal_places",
    "mass_property_decimal_places",
)


@dataclass(frozen=True, slots=True)
class DocumentUnits:
    unit_system: UnitSetting | None = None
    length: UnitSetting | None = None
    length_decimal_places: SourcedValue[int] | None = None
    dual_length: UnitSetting | None = None
    dual_length_decimal_places: SourcedValue[int] | None = None
    angle: UnitSetting | None = None
    angle_decimal_places: SourcedValue[int] | None = None
    mass_property_length: UnitSetting | None = None
    mass_property_decimal_places: SourcedValue[int] | None = None
    mass: UnitSetting | None = None
    volume: UnitSetting | None = None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DocumentUnits:
        settings = {
            name: (
                None if value.get(name) is None else UnitSetting.from_dict(value[name])
            )
            for name in _UNIT_SETTING_FIELDS
        }
        places = {
            name: _optional_sourced(value.get(name), int)
            for name in _UNIT_PLACES_FIELDS
        }
        return cls(**settings, **places)

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for name in (*_UNIT_SETTING_FIELDS, *_UNIT_PLACES_FIELDS):
            item = getattr(self, name)
            if item is not None:
                result[name] = item.to_dict()
        return result


@dataclass(frozen=True, slots=True)
class FeatureDimension:
    name: str | None
    raw_value: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> FeatureDimension:
        return cls(
            name=_optional_str(value.get("name")), raw_value=str(value["raw_value"])
        )

    def to_dict(self) -> dict[str, Any]:
        return {"name": self.name, "raw_value": self.raw_value}


@dataclass(frozen=True, slots=True)
class FeatureRecord:
    source_tag: str
    source_id: str | None
    name: SourcedValue[str] | None
    source_type: str | None
    dissectable: bool | None
    dissectable_root: bool | None
    dissectable_children: tuple[str, ...]
    dimensions: tuple[FeatureDimension, ...]
    raw_attributes: Mapping[str, str]
    stream_path: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> FeatureRecord:
        return cls(
            source_tag=str(value["source_tag"]),
            source_id=_optional_str(value.get("source_id")),
            name=_optional_sourced(value.get("name"), str),
            source_type=_optional_str(value.get("source_type")),
            dissectable=_optional_bool(value.get("dissectable")),
            dissectable_root=_optional_bool(value.get("dissectable_root")),
            dissectable_children=tuple(
                str(item) for item in value.get("dissectable_children", [])
            ),
            dimensions=tuple(
                FeatureDimension.from_dict(item) for item in value.get("dimensions", [])
            ),
            raw_attributes={
                str(key): str(item)
                for key, item in value.get("raw_attributes", {}).items()
            },
            stream_path=str(value["stream_path"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "source_tag": self.source_tag,
            "source_id": self.source_id,
            "name": _sourced_dict(self.name),
            "source_type": self.source_type,
            "dissectable": self.dissectable,
            "dissectable_root": self.dissectable_root,
            "stream_path": self.stream_path,
        }
        if self.dissectable_children:
            result["dissectable_children"] = list(self.dissectable_children)
        if self.dimensions:
            result["dimensions"] = [item.to_dict() for item in self.dimensions]
        if self.raw_attributes:
            result["raw_attributes"] = dict(self.raw_attributes)
        return result


@dataclass(frozen=True, slots=True)
class EmbeddedDocument:
    role: EmbeddedDocumentRole
    name: str
    entry_id: str
    stream_path: str
    byte_len: int | None
    sha256: str | None
    envelope: Envelope | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> EmbeddedDocument:
        envelope = value.get("envelope")
        return cls(
            role=EmbeddedDocumentRole(value["role"]),
            name=str(value["name"]),
            entry_id=str(value["entry_id"]),
            stream_path=str(value["stream_path"]),
            byte_len=_optional_int(value.get("byte_len")),
            sha256=_optional_str(value.get("sha256")),
            envelope=None if envelope is None else Envelope(envelope),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "role": self.role.value,
            "name": self.name,
            "entry_id": self.entry_id,
            "stream_path": self.stream_path,
            "byte_len": self.byte_len,
            "sha256": self.sha256,
            "envelope": None if self.envelope is None else self.envelope.value,
        }


@dataclass(frozen=True, slots=True)
class DrawingRecordSource:
    entry_id: str
    stream_path: str
    decoded_offset: int
    byte_len: int
    sha256: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingRecordSource:
        return cls(
            entry_id=str(value["entry_id"]),
            stream_path=str(value["stream_path"]),
            decoded_offset=int(value["decoded_offset"]),
            byte_len=int(value["byte_len"]),
            sha256=str(value["sha256"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "entry_id": self.entry_id,
            "stream_path": self.stream_path,
            "decoded_offset": self.decoded_offset,
            "byte_len": self.byte_len,
            "sha256": self.sha256,
        }


@dataclass(frozen=True, slots=True)
class DrawingRecord:
    id: str
    record_class: DrawingRecordClass
    source_tag: str
    parent_id: str | None
    source_id: str | None
    name: str | None
    source_type: str | None
    source_attributes: Mapping[str, str]
    direct_text: str | None
    source: DrawingRecordSource

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingRecord:
        return cls(
            id=str(value["id"]),
            record_class=DrawingRecordClass(value["class"]),
            source_tag=str(value["source_tag"]),
            parent_id=_optional_str(value.get("parent_id")),
            source_id=_optional_str(value.get("source_id")),
            name=_optional_str(value.get("name")),
            source_type=_optional_str(value.get("source_type")),
            source_attributes={
                str(name): str(item)
                for name, item in value.get("source_attributes", {}).items()
            },
            direct_text=_optional_str(value.get("direct_text")),
            source=DrawingRecordSource.from_dict(value["source"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "class": self.record_class.value,
            "source_tag": self.source_tag,
            "parent_id": self.parent_id,
            "source_id": self.source_id,
            "name": self.name,
            "source_type": self.source_type,
            "direct_text": self.direct_text,
            "source": self.source.to_dict(),
        }
        if self.source_attributes:
            result["source_attributes"] = dict(self.source_attributes)
        return result


@dataclass(frozen=True, slots=True)
class DrawingStructureSheet:
    record_id: str
    source_id: str | None
    name: str | None
    source_type: str | None
    view_record_ids: tuple[str, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingStructureSheet:
        return cls(
            record_id=str(value["record_id"]),
            source_id=_optional_str(value.get("source_id")),
            name=_optional_str(value.get("name")),
            source_type=_optional_str(value.get("source_type")),
            view_record_ids=tuple(
                str(item) for item in value.get("view_record_ids", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "record_id": self.record_id,
            "source_id": self.source_id,
            "name": self.name,
            "source_type": self.source_type,
            "view_record_ids": list(self.view_record_ids),
        }


@dataclass(frozen=True, slots=True)
class DrawingStructureView:
    record_id: str
    sheet_record_id: str | None
    source_id: str | None
    name: str | None
    referenced_document: str | None
    referenced_configuration: str | None
    parent_view_record_id: str | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingStructureView:
        return cls(
            record_id=str(value["record_id"]),
            sheet_record_id=_optional_str(value.get("sheet_record_id")),
            source_id=_optional_str(value.get("source_id")),
            name=_optional_str(value.get("name")),
            referenced_document=_optional_str(value.get("referenced_document")),
            referenced_configuration=_optional_str(
                value.get("referenced_configuration")
            ),
            parent_view_record_id=_optional_str(value.get("parent_view_record_id")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "record_id": self.record_id,
            "sheet_record_id": self.sheet_record_id,
            "source_id": self.source_id,
            "name": self.name,
            "referenced_document": self.referenced_document,
            "referenced_configuration": self.referenced_configuration,
            "parent_view_record_id": self.parent_view_record_id,
        }


@dataclass(frozen=True, slots=True)
class DrawingCarrier:
    entry_id: str
    stream_path: str
    role: DrawingCarrierRole
    decoded_size: int
    decoded_sha256: str
    record_framing_verified: bool

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingCarrier:
        return cls(
            entry_id=str(value["entry_id"]),
            stream_path=str(value["stream_path"]),
            role=DrawingCarrierRole(value["role"]),
            decoded_size=int(value["decoded_size"]),
            decoded_sha256=str(value["decoded_sha256"]),
            record_framing_verified=bool(value["record_framing_verified"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "entry_id": self.entry_id,
            "stream_path": self.stream_path,
            "role": self.role.value,
            "decoded_size": self.decoded_size,
            "decoded_sha256": self.decoded_sha256,
            "record_framing_verified": self.record_framing_verified,
        }


@dataclass(frozen=True, slots=True)
class DrawingStructureCoverage:
    record_count: int
    record_class_counts: Mapping[str, int]
    sheet_record_count: int
    supported_sheet_count: int
    sheet_view_count: int
    unassigned_view_record_count: int
    candidate_stream_count: int
    candidate_stream_bytes: int
    located_record_count: int
    unique_record_range_count: int
    partition_status: DrawingBytePartitionStatus
    typed_bytes: int | None
    uninterpreted_bytes: int | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingStructureCoverage:
        return cls(
            record_count=int(value["record_count"]),
            record_class_counts={
                str(name): int(item)
                for name, item in value.get("record_class_counts", {}).items()
            },
            sheet_record_count=int(value["sheet_record_count"]),
            supported_sheet_count=int(value["supported_sheet_count"]),
            sheet_view_count=int(value["sheet_view_count"]),
            unassigned_view_record_count=int(value["unassigned_view_record_count"]),
            candidate_stream_count=int(value["candidate_stream_count"]),
            candidate_stream_bytes=int(value["candidate_stream_bytes"]),
            located_record_count=int(value["located_record_count"]),
            unique_record_range_count=int(value["unique_record_range_count"]),
            partition_status=DrawingBytePartitionStatus(value["partition_status"]),
            typed_bytes=_optional_int(value.get("typed_bytes")),
            uninterpreted_bytes=_optional_int(value.get("uninterpreted_bytes")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "record_count": self.record_count,
            "record_class_counts": dict(self.record_class_counts),
            "sheet_record_count": self.sheet_record_count,
            "supported_sheet_count": self.supported_sheet_count,
            "sheet_view_count": self.sheet_view_count,
            "unassigned_view_record_count": self.unassigned_view_record_count,
            "candidate_stream_count": self.candidate_stream_count,
            "candidate_stream_bytes": self.candidate_stream_bytes,
            "located_record_count": self.located_record_count,
            "unique_record_range_count": self.unique_record_range_count,
            "partition_status": self.partition_status.value,
            "typed_bytes": self.typed_bytes,
            "uninterpreted_bytes": self.uninterpreted_bytes,
        }


@dataclass(frozen=True, slots=True)
class DrawingStructureDocument:
    source: SourceInfo
    internal_version: SourcedValue[int] | None
    records: tuple[DrawingRecord, ...]
    sheets: tuple[DrawingStructureSheet, ...]
    views: tuple[DrawingStructureView, ...]
    source_streams: tuple[DrawingCarrier, ...]
    coverage: DrawingStructureCoverage

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingStructureDocument:
        return cls(
            source=SourceInfo.from_dict(value["source"]),
            internal_version=_optional_sourced(value.get("internal_version"), int),
            records=tuple(
                DrawingRecord.from_dict(item) for item in value.get("records", [])
            ),
            sheets=tuple(
                DrawingStructureSheet.from_dict(item)
                for item in value.get("sheets", [])
            ),
            views=tuple(
                DrawingStructureView.from_dict(item) for item in value.get("views", [])
            ),
            source_streams=tuple(
                DrawingCarrier.from_dict(item)
                for item in value.get("source_streams", [])
            ),
            coverage=DrawingStructureCoverage.from_dict(value["coverage"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source": self.source.to_dict(),
            "internal_version": _sourced_dict(self.internal_version),
            "records": [item.to_dict() for item in self.records],
            "sheets": [item.to_dict() for item in self.sheets],
            "views": [item.to_dict() for item in self.views],
            "source_streams": [item.to_dict() for item in self.source_streams],
            "coverage": self.coverage.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class DrawingStructureResult:
    status: DrawingStructureStatus
    structure: DrawingStructureDocument | None
    diagnostics: tuple[Diagnostic, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingStructureResult:
        structure = value.get("structure")
        return cls(
            status=DrawingStructureStatus(value["status"]),
            structure=(
                None
                if structure is None
                else DrawingStructureDocument.from_dict(structure)
            ),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "structure": None if self.structure is None else self.structure.to_dict(),
            "diagnostics": [item.to_dict() for item in self.diagnostics],
        }


@dataclass(frozen=True, slots=True)
class UnknownRecord:
    entry_id: str | None
    stream_path: str | None
    record_kind: int | None
    offset_basis: RecordOffsetBasis
    offset: int
    length: int
    sha256: str
    reason_code: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> UnknownRecord:
        return cls(
            entry_id=_optional_str(value.get("entry_id")),
            stream_path=_optional_str(value.get("stream_path")),
            record_kind=_optional_int(value.get("record_kind")),
            offset_basis=RecordOffsetBasis(value["offset_basis"]),
            offset=int(value["offset"]),
            length=int(value["length"]),
            sha256=str(value["sha256"]),
            reason_code=str(value["reason_code"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "entry_id": self.entry_id,
            "stream_path": self.stream_path,
            "record_kind": self.record_kind,
            "offset_basis": self.offset_basis.value,
            "offset": self.offset,
            "length": self.length,
            "sha256": self.sha256,
            "reason_code": self.reason_code,
        }


@dataclass(frozen=True, slots=True)
class GeometryStreamCandidate:
    entry_id: str
    stream_path: str
    role: GeometryStreamRole
    selection: GeometryStreamSelection
    decoded_size: int | None
    decoded_sha256: str | None
    selection_evidence: tuple[str, ...] = ()

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryStreamCandidate:
        return cls(
            entry_id=str(value["entry_id"]),
            stream_path=str(value["stream_path"]),
            role=GeometryStreamRole(value["role"]),
            selection=GeometryStreamSelection(value["selection"]),
            decoded_size=_optional_int(value.get("decoded_size")),
            decoded_sha256=_optional_str(value.get("decoded_sha256")),
            selection_evidence=tuple(
                str(item) for item in value.get("selection_evidence", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {
            "entry_id": self.entry_id,
            "stream_path": self.stream_path,
            "role": self.role.value,
            "selection": self.selection.value,
            "decoded_size": self.decoded_size,
            "decoded_sha256": self.decoded_sha256,
        }
        if self.selection_evidence:
            result["selection_evidence"] = list(self.selection_evidence)
        return result


@dataclass(frozen=True, slots=True)
class GeometryEntityProvenance:
    stream: str | None
    offset: int | None
    tag: str | None
    exactness: GeometryExactness
    field_exactness: Mapping[str, GeometryExactness] = field(default_factory=dict)

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryEntityProvenance:
        return cls(
            stream=_optional_str(value.get("stream")),
            offset=_optional_int(value.get("offset")),
            tag=_optional_str(value.get("tag")),
            exactness=GeometryExactness(value["exactness"]),
            field_exactness={
                str(name): GeometryExactness(item)
                for name, item in value.get("field_exactness", {}).items()
            },
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "stream": self.stream,
            "offset": self.offset,
            "tag": self.tag,
            "exactness": self.exactness.value,
        }
        if self.field_exactness:
            result["field_exactness"] = {
                name: item.value for name, item in self.field_exactness.items()
            }
        return result


@dataclass(frozen=True, slots=True)
class GeometrySourceObject:
    format: str
    object_id: str
    name: str | None
    color: tuple[float, float, float, float] | None
    visible: bool | None
    layer: str | None
    instance_path: tuple[str, ...] = ()

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometrySourceObject:
        return cls(
            format=str(value["format"]),
            object_id=str(value["object_id"]),
            name=_optional_str(value.get("name")),
            color=_optional_float_quad(value.get("color")),
            visible=_optional_bool(value.get("visible")),
            layer=_optional_str(value.get("layer")),
            instance_path=tuple(str(item) for item in value.get("instance_path", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "format": self.format,
            "object_id": self.object_id,
            "name": self.name,
            "color": None if self.color is None else list(self.color),
            "visible": self.visible,
            "layer": self.layer,
        }
        if self.instance_path:
            result["instance_path"] = list(self.instance_path)
        return result


@dataclass(frozen=True, slots=True)
class GeometryBody:
    id: str
    kind: str
    region_ids: tuple[str, ...]
    transform: Any | None
    name: str | None
    color: tuple[float, float, float, float] | None
    visible: bool | None
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryBody:
        return cls(
            id=str(value["id"]),
            kind=str(value["kind"]),
            region_ids=tuple(str(item) for item in value.get("region_ids", [])),
            transform=value.get("transform"),
            name=_optional_str(value.get("name")),
            color=_optional_float_quad(value.get("color")),
            visible=_optional_bool(value.get("visible")),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "kind": self.kind,
            "region_ids": list(self.region_ids),
            "transform": self.transform,
            "name": self.name,
            "color": None if self.color is None else list(self.color),
            "visible": self.visible,
            "provenance": self.provenance.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class GeometryRegion:
    id: str
    body_id: str
    shell_ids: tuple[str, ...]
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryRegion:
        return cls(
            id=str(value["id"]),
            body_id=str(value["body_id"]),
            shell_ids=tuple(str(item) for item in value.get("shell_ids", [])),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "body_id": self.body_id,
            "shell_ids": list(self.shell_ids),
            "provenance": self.provenance.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class GeometryShell:
    id: str
    region_id: str
    face_ids: tuple[str, ...]
    wire_edge_ids: tuple[str, ...]
    free_vertex_ids: tuple[str, ...]
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryShell:
        return cls(
            id=str(value["id"]),
            region_id=str(value["region_id"]),
            face_ids=tuple(str(item) for item in value.get("face_ids", [])),
            wire_edge_ids=tuple(str(item) for item in value.get("wire_edge_ids", [])),
            free_vertex_ids=tuple(
                str(item) for item in value.get("free_vertex_ids", [])
            ),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "region_id": self.region_id,
            "face_ids": list(self.face_ids),
            "provenance": self.provenance.to_dict(),
        }
        if self.wire_edge_ids:
            result["wire_edge_ids"] = list(self.wire_edge_ids)
        if self.free_vertex_ids:
            result["free_vertex_ids"] = list(self.free_vertex_ids)
        return result


@dataclass(frozen=True, slots=True)
class GeometryFace:
    id: str
    shell_id: str
    surface_id: str
    sense: str
    loop_ids: tuple[str, ...]
    name: str | None
    color: tuple[float, float, float, float] | None
    tolerance: float | None
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryFace:
        return cls(
            id=str(value["id"]),
            shell_id=str(value["shell_id"]),
            surface_id=str(value["surface_id"]),
            sense=str(value["sense"]),
            loop_ids=tuple(str(item) for item in value.get("loop_ids", [])),
            name=_optional_str(value.get("name")),
            color=_optional_float_quad(value.get("color")),
            tolerance=_optional_float(value.get("tolerance")),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "shell_id": self.shell_id,
            "surface_id": self.surface_id,
            "sense": self.sense,
            "loop_ids": list(self.loop_ids),
            "name": self.name,
            "color": None if self.color is None else list(self.color),
            "tolerance": self.tolerance,
            "provenance": self.provenance.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class GeometryPcurveUse:
    pcurve_id: str
    isoparametric: bool | None
    parameter_range: tuple[float, float] | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryPcurveUse:
        return cls(
            pcurve_id=str(value["pcurve_id"]),
            isoparametric=_optional_bool(value.get("isoparametric")),
            parameter_range=_optional_float_pair(value.get("parameter_range")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "pcurve_id": self.pcurve_id,
            "isoparametric": self.isoparametric,
            "parameter_range": (
                None if self.parameter_range is None else list(self.parameter_range)
            ),
        }


@dataclass(frozen=True, slots=True)
class GeometryVertexUse:
    vertex_id: str
    after_coedge_id: str | None
    pcurves: tuple[GeometryPcurveUse, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryVertexUse:
        return cls(
            vertex_id=str(value["vertex_id"]),
            after_coedge_id=_optional_str(value.get("after_coedge_id")),
            pcurves=tuple(
                GeometryPcurveUse.from_dict(item) for item in value.get("pcurves", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "vertex_id": self.vertex_id,
            "after_coedge_id": self.after_coedge_id,
        }
        if self.pcurves:
            result["pcurves"] = [item.to_dict() for item in self.pcurves]
        return result


class GeometryLoopRoleMethod(str, Enum):
    PLANAR_ANALYTIC_WINDING = "planar_analytic_winding"
    CYLINDRICAL_ANALYTIC_CHART = "cylindrical_analytic_chart"


@dataclass(frozen=True, slots=True)
class GeometryDerivedLoopRole:
    """Geometric loop classification, separate from stored source fields."""

    role: str
    method: GeometryLoopRoleMethod
    signed_area_mm2: float
    tolerance_mm: float

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryDerivedLoopRole:
        return cls(
            str(value["role"]),
            GeometryLoopRoleMethod(value["method"]),
            float(value["signed_area_mm2"]),
            float(value["tolerance_mm"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "role": self.role,
            "method": self.method.value,
            "signed_area_mm2": self.signed_area_mm2,
            "tolerance_mm": self.tolerance_mm,
        }


@dataclass(frozen=True, slots=True)
class GeometryLoop:
    id: str
    face_id: str
    boundary_role: str
    coedge_ids: tuple[str, ...]
    vertex_uses: tuple[GeometryVertexUse, ...]
    provenance: GeometryEntityProvenance
    derived_boundary_role: GeometryDerivedLoopRole | None = None

    @property
    def effective_boundary_role(self) -> str:
        if self.boundary_role != "unspecified":
            return self.boundary_role
        if self.derived_boundary_role is not None:
            return self.derived_boundary_role.role
        return self.boundary_role

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryLoop:
        return cls(
            id=str(value["id"]),
            face_id=str(value["face_id"]),
            boundary_role=str(value["boundary_role"]),
            derived_boundary_role=(
                GeometryDerivedLoopRole.from_dict(value["derived_boundary_role"])
                if value.get("derived_boundary_role") is not None
                else None
            ),
            coedge_ids=tuple(str(item) for item in value.get("coedge_ids", [])),
            vertex_uses=tuple(
                GeometryVertexUse.from_dict(item)
                for item in value.get("vertex_uses", [])
            ),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "face_id": self.face_id,
            "boundary_role": self.boundary_role,
            "provenance": self.provenance.to_dict(),
        }
        if self.coedge_ids:
            result["coedge_ids"] = list(self.coedge_ids)
        if self.vertex_uses:
            result["vertex_uses"] = [item.to_dict() for item in self.vertex_uses]
        if self.derived_boundary_role is not None:
            result["derived_boundary_role"] = self.derived_boundary_role.to_dict()
        return result


@dataclass(frozen=True, slots=True)
class GeometryCoedge:
    id: str
    loop_id: str
    edge_id: str
    next_id: str
    previous_id: str
    radial_next_id: str
    sense: str
    pcurves: tuple[GeometryPcurveUse, ...]
    use_curve_id: str | None
    use_curve_parameter_range: tuple[float, float] | None
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryCoedge:
        return cls(
            id=str(value["id"]),
            loop_id=str(value["loop_id"]),
            edge_id=str(value["edge_id"]),
            next_id=str(value["next_id"]),
            previous_id=str(value["previous_id"]),
            radial_next_id=str(value["radial_next_id"]),
            sense=str(value["sense"]),
            pcurves=tuple(
                GeometryPcurveUse.from_dict(item) for item in value.get("pcurves", [])
            ),
            use_curve_id=_optional_str(value.get("use_curve_id")),
            use_curve_parameter_range=_optional_float_pair(
                value.get("use_curve_parameter_range")
            ),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "loop_id": self.loop_id,
            "edge_id": self.edge_id,
            "next_id": self.next_id,
            "previous_id": self.previous_id,
            "radial_next_id": self.radial_next_id,
            "sense": self.sense,
            "use_curve_id": self.use_curve_id,
            "use_curve_parameter_range": (
                None
                if self.use_curve_parameter_range is None
                else list(self.use_curve_parameter_range)
            ),
            "provenance": self.provenance.to_dict(),
        }
        if self.pcurves:
            result["pcurves"] = [item.to_dict() for item in self.pcurves]
        return result


class GeometryIntervalMethod(str, Enum):
    LINE_PROJECTION = "line_projection"
    NURBS_SUPPORT_ENDPOINTS = "nurbs_support_endpoints"
    NURBS_MONOTONE_PROJECTION = "nurbs_monotone_projection"
    CONIC_ENDPOINTS = "conic_endpoints"
    CLOSED_CIRCLE_SEAM = "closed_circle_seam"


@dataclass(frozen=True, slots=True)
class GeometryDerivedInterval:
    """Endpoint-derived parameters; these do not certify stored source trim."""

    parameter_range: tuple[float, float]
    method: GeometryIntervalMethod
    tolerance_mm: float
    max_endpoint_error_mm: float

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryDerivedInterval:
        bounds = value["parameter_range"]
        return cls(
            parameter_range=(float(bounds[0]), float(bounds[1])),
            method=GeometryIntervalMethod(value["method"]),
            tolerance_mm=float(value["tolerance_mm"]),
            max_endpoint_error_mm=float(value["max_endpoint_error_mm"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "parameter_range": list(self.parameter_range),
            "method": self.method.value,
            "tolerance_mm": self.tolerance_mm,
            "max_endpoint_error_mm": self.max_endpoint_error_mm,
        }


@dataclass(frozen=True, slots=True)
class GeometryEdge:
    id: str
    curve_id: str | None
    start_vertex_id: str
    end_vertex_id: str
    parameter_range: tuple[float, float] | None
    tolerance: float | None
    provenance: GeometryEntityProvenance
    derived_parameter_interval: GeometryDerivedInterval | None = None

    @property
    def effective_parameter_range(self) -> tuple[float, float] | None:
        """Prefer decoded parameters, then the explicitly derived interval."""
        if self.parameter_range is not None:
            return self.parameter_range
        if self.derived_parameter_interval is not None:
            return self.derived_parameter_interval.parameter_range
        return None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryEdge:
        return cls(
            id=str(value["id"]),
            curve_id=_optional_str(value.get("curve_id")),
            start_vertex_id=str(value["start_vertex_id"]),
            end_vertex_id=str(value["end_vertex_id"]),
            parameter_range=_optional_float_pair(value.get("parameter_range")),
            tolerance=_optional_float(value.get("tolerance")),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
            derived_parameter_interval=(
                GeometryDerivedInterval.from_dict(value["derived_parameter_interval"])
                if value.get("derived_parameter_interval") is not None
                else None
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {
            "id": self.id,
            "curve_id": self.curve_id,
            "start_vertex_id": self.start_vertex_id,
            "end_vertex_id": self.end_vertex_id,
            "parameter_range": (
                None if self.parameter_range is None else list(self.parameter_range)
            ),
            "tolerance": self.tolerance,
            "provenance": self.provenance.to_dict(),
        }
        if self.derived_parameter_interval is not None:
            result["derived_parameter_interval"] = (
                self.derived_parameter_interval.to_dict()
            )
        return result


@dataclass(frozen=True, slots=True)
class GeometryVertex:
    id: str
    point_id: str
    tolerance: float | None
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryVertex:
        return cls(
            id=str(value["id"]),
            point_id=str(value["point_id"]),
            tolerance=_optional_float(value.get("tolerance")),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "point_id": self.point_id,
            "tolerance": self.tolerance,
            "provenance": self.provenance.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class GeometryPoint:
    id: str
    position: tuple[float, float, float]
    source_object: GeometrySourceObject | None
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryPoint:
        return cls(
            id=str(value["id"]),
            position=_float_triple(value["position"]),
            source_object=(
                None
                if value.get("source_object") is None
                else GeometrySourceObject.from_dict(value["source_object"])
            ),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "position": list(self.position),
            "source_object": (
                None if self.source_object is None else self.source_object.to_dict()
            ),
            "provenance": self.provenance.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class GeometryPcurveState:
    wrapper_reversed: bool | None
    native_tail_flags: tuple[bool, bool, bool, bool] | None
    parameter_range: tuple[float, float] | None
    fit_tolerance: float | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryPcurveState:
        return cls(
            wrapper_reversed=_optional_bool(value.get("wrapper_reversed")),
            native_tail_flags=_optional_bool_quad(value.get("native_tail_flags")),
            parameter_range=_optional_float_pair(value.get("parameter_range")),
            fit_tolerance=_optional_float(value.get("fit_tolerance")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "wrapper_reversed": self.wrapper_reversed,
            "native_tail_flags": (
                None if self.native_tail_flags is None else list(self.native_tail_flags)
            ),
            "parameter_range": (
                None if self.parameter_range is None else list(self.parameter_range)
            ),
            "fit_tolerance": self.fit_tolerance,
        }


@dataclass(frozen=True, slots=True)
class GeometryCarrier:
    id: str
    domain: GeometryCarrierDomain
    kind: str
    definition: Any
    raw_record_id: str | None
    source_object: GeometrySourceObject | None
    pcurve_state: GeometryPcurveState | None
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryCarrier:
        return cls(
            id=str(value["id"]),
            domain=GeometryCarrierDomain(value["domain"]),
            kind=str(value["kind"]),
            definition=value["definition"],
            raw_record_id=_optional_str(value.get("raw_record_id")),
            source_object=(
                None
                if value.get("source_object") is None
                else GeometrySourceObject.from_dict(value["source_object"])
            ),
            pcurve_state=(
                None
                if value.get("pcurve_state") is None
                else GeometryPcurveState.from_dict(value["pcurve_state"])
            ),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "domain": self.domain.value,
            "kind": self.kind,
            "definition": self.definition,
            "raw_record_id": self.raw_record_id,
            "source_object": (
                None if self.source_object is None else self.source_object.to_dict()
            ),
            "pcurve_state": (
                None if self.pcurve_state is None else self.pcurve_state.to_dict()
            ),
            "provenance": self.provenance.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class GeometryConstruction:
    id: str
    domain: GeometryConstructionDomain
    produced_carrier_id: str
    definition: Any
    cache_fit_tolerance: float | None
    record_bounds: tuple[float | None, float | None, float | None, float | None] | None
    raw_record_id: str | None
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryConstruction:
        return cls(
            id=str(value["id"]),
            domain=GeometryConstructionDomain(value["domain"]),
            produced_carrier_id=str(value["produced_carrier_id"]),
            definition=value["definition"],
            cache_fit_tolerance=_optional_float(value.get("cache_fit_tolerance")),
            record_bounds=_optional_optional_float_quad(value.get("record_bounds")),
            raw_record_id=_optional_str(value.get("raw_record_id")),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "domain": self.domain.value,
            "produced_carrier_id": self.produced_carrier_id,
            "definition": self.definition,
            "cache_fit_tolerance": self.cache_fit_tolerance,
            "record_bounds": (
                None if self.record_bounds is None else list(self.record_bounds)
            ),
            "raw_record_id": self.raw_record_id,
            "provenance": self.provenance.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class GeometryTessellationChannel:
    domain: str
    item_size: int
    kind: int
    flags: int
    count: int
    byte_len: int
    sha256: str
    indices: tuple[int, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryTessellationChannel:
        return cls(
            domain=str(value["domain"]),
            item_size=int(value["item_size"]),
            kind=int(value["kind"]),
            flags=int(value["flags"]),
            count=int(value["count"]),
            byte_len=int(value["byte_len"]),
            sha256=str(value["sha256"]),
            indices=tuple(int(item) for item in value.get("indices", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "domain": self.domain,
            "item_size": self.item_size,
            "kind": self.kind,
            "flags": self.flags,
            "count": self.count,
            "byte_len": self.byte_len,
            "sha256": self.sha256,
        }
        if self.indices:
            result["indices"] = list(self.indices)
        return result


@dataclass(frozen=True, slots=True)
class GeometryTessellationTriangleGroup:
    source_id: str | None
    triangles: tuple[int, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryTessellationTriangleGroup:
        return cls(
            source_id=_optional_str(value.get("source_id")),
            triangles=tuple(int(item) for item in value.get("triangles", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return {"source_id": self.source_id, "triangles": list(self.triangles)}


@dataclass(frozen=True, slots=True)
class GeometryTessellationTextureAssignment:
    source_id: str | None
    texture_id: str
    triangles: tuple[int, ...]

    @classmethod
    def from_dict(
        cls, value: Mapping[str, Any]
    ) -> GeometryTessellationTextureAssignment:
        return cls(
            source_id=_optional_str(value.get("source_id")),
            texture_id=str(value["texture_id"]),
            triangles=tuple(int(item) for item in value.get("triangles", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source_id": self.source_id,
            "texture_id": self.texture_id,
            "triangles": list(self.triangles),
        }


@dataclass(frozen=True, slots=True)
class GeometryTessellation:
    id: str
    body_id: str | None
    face_ids: tuple[str, ...]
    chordal_deflection: float | None
    source_object: GeometrySourceObject | None
    vertices: tuple[tuple[float, float, float], ...]
    triangles: tuple[tuple[int, int, int], ...]
    feature_edges: tuple[tuple[int, int], ...]
    strip_lengths: tuple[int, ...]
    normals: tuple[tuple[float, float, float], ...]
    corner_normals: tuple[tuple[float, float, float], ...]
    triangle_groups: tuple[GeometryTessellationTriangleGroup, ...]
    texture_assignments: tuple[GeometryTessellationTextureAssignment, ...]
    channels: tuple[GeometryTessellationChannel, ...]
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryTessellation:
        return cls(
            id=str(value["id"]),
            body_id=_optional_str(value.get("body_id")),
            face_ids=tuple(str(item) for item in value.get("face_ids", [])),
            chordal_deflection=_optional_float(value.get("chordal_deflection")),
            source_object=(
                None
                if value.get("source_object") is None
                else GeometrySourceObject.from_dict(value["source_object"])
            ),
            vertices=tuple(_float_triple(item) for item in value.get("vertices", [])),
            triangles=tuple(_int_triple(item) for item in value.get("triangles", [])),
            feature_edges=tuple(
                _int_pair(item) for item in value.get("feature_edges", [])
            ),
            strip_lengths=tuple(int(item) for item in value.get("strip_lengths", [])),
            normals=tuple(_float_triple(item) for item in value.get("normals", [])),
            corner_normals=tuple(
                _float_triple(item) for item in value.get("corner_normals", [])
            ),
            triangle_groups=tuple(
                GeometryTessellationTriangleGroup.from_dict(item)
                for item in value.get("triangle_groups", [])
            ),
            texture_assignments=tuple(
                GeometryTessellationTextureAssignment.from_dict(item)
                for item in value.get("texture_assignments", [])
            ),
            channels=tuple(
                GeometryTessellationChannel.from_dict(item)
                for item in value.get("channels", [])
            ),
            provenance=GeometryEntityProvenance.from_dict(value["provenance"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "body_id": self.body_id,
            "chordal_deflection": self.chordal_deflection,
            "source_object": (
                None if self.source_object is None else self.source_object.to_dict()
            ),
            "vertices": [list(item) for item in self.vertices],
            "triangles": [list(item) for item in self.triangles],
            "provenance": self.provenance.to_dict(),
        }
        if self.face_ids:
            result["face_ids"] = list(self.face_ids)
        if self.feature_edges:
            result["feature_edges"] = [list(item) for item in self.feature_edges]
        if self.strip_lengths:
            result["strip_lengths"] = list(self.strip_lengths)
        if self.normals:
            result["normals"] = [list(item) for item in self.normals]
        if self.corner_normals:
            result["corner_normals"] = [list(item) for item in self.corner_normals]
        if self.triangle_groups:
            result["triangle_groups"] = [
                item.to_dict() for item in self.triangle_groups
            ]
        if self.texture_assignments:
            result["texture_assignments"] = [
                item.to_dict() for item in self.texture_assignments
            ]
        if self.channels:
            result["channels"] = [item.to_dict() for item in self.channels]
        return result


@dataclass(frozen=True, slots=True)
class GeometryConfigurationState:
    id: str
    ordinal: int
    active: bool | None
    source_index: int | None
    name: str | None
    body_ids: tuple[str, ...] | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryConfigurationState:
        bodies = value.get("body_ids")
        return cls(
            id=str(value["id"]),
            ordinal=int(value["ordinal"]),
            active=_optional_bool(value.get("active")),
            source_index=_optional_int(value.get("source_index")),
            name=_optional_str(value.get("name")),
            body_ids=(None if bodies is None else tuple(str(item) for item in bodies)),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "ordinal": self.ordinal,
            "active": self.active,
            "source_index": self.source_index,
            "name": self.name,
            "body_ids": None if self.body_ids is None else list(self.body_ids),
        }


@dataclass(frozen=True, slots=True)
class GeometryTopologyMetrics:
    body_id: str
    regions: int
    shells: int
    faces: int
    loops: int
    coedges: int
    edges: int
    vertices: int
    euler_characteristic: int | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryTopologyMetrics:
        return cls(
            body_id=str(value["body_id"]),
            regions=int(value["regions"]),
            shells=int(value["shells"]),
            faces=int(value["faces"]),
            loops=int(value["loops"]),
            coedges=int(value["coedges"]),
            edges=int(value["edges"]),
            vertices=int(value["vertices"]),
            euler_characteristic=_optional_int(value.get("euler_characteristic")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in self.__dataclass_fields__}


@dataclass(frozen=True, slots=True)
class GeometryModel:
    bodies: tuple[GeometryBody, ...]
    regions: tuple[GeometryRegion, ...]
    shells: tuple[GeometryShell, ...]
    faces: tuple[GeometryFace, ...]
    loops: tuple[GeometryLoop, ...]
    coedges: tuple[GeometryCoedge, ...]
    edges: tuple[GeometryEdge, ...]
    vertices: tuple[GeometryVertex, ...]
    points: tuple[GeometryPoint, ...]
    carriers: tuple[GeometryCarrier, ...]
    constructions: tuple[GeometryConstruction, ...]
    tessellations: tuple[GeometryTessellation, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryModel:
        return cls(
            bodies=tuple(
                GeometryBody.from_dict(item) for item in value.get("bodies", [])
            ),
            regions=tuple(
                GeometryRegion.from_dict(item) for item in value.get("regions", [])
            ),
            shells=tuple(
                GeometryShell.from_dict(item) for item in value.get("shells", [])
            ),
            faces=tuple(
                GeometryFace.from_dict(item) for item in value.get("faces", [])
            ),
            loops=tuple(
                GeometryLoop.from_dict(item) for item in value.get("loops", [])
            ),
            coedges=tuple(
                GeometryCoedge.from_dict(item) for item in value.get("coedges", [])
            ),
            edges=tuple(
                GeometryEdge.from_dict(item) for item in value.get("edges", [])
            ),
            vertices=tuple(
                GeometryVertex.from_dict(item) for item in value.get("vertices", [])
            ),
            points=tuple(
                GeometryPoint.from_dict(item) for item in value.get("points", [])
            ),
            carriers=tuple(
                GeometryCarrier.from_dict(item) for item in value.get("carriers", [])
            ),
            constructions=tuple(
                GeometryConstruction.from_dict(item)
                for item in value.get("constructions", [])
            ),
            tessellations=tuple(
                GeometryTessellation.from_dict(item)
                for item in value.get("tessellations", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "bodies": [item.to_dict() for item in self.bodies],
            "regions": [item.to_dict() for item in self.regions],
            "shells": [item.to_dict() for item in self.shells],
            "faces": [item.to_dict() for item in self.faces],
            "loops": [item.to_dict() for item in self.loops],
            "coedges": [item.to_dict() for item in self.coedges],
            "edges": [item.to_dict() for item in self.edges],
            "vertices": [item.to_dict() for item in self.vertices],
            "points": [item.to_dict() for item in self.points],
            "carriers": [item.to_dict() for item in self.carriers],
            "constructions": [item.to_dict() for item in self.constructions],
            "tessellations": [item.to_dict() for item in self.tessellations],
        }


@dataclass(frozen=True, slots=True)
class GeometryRawRecord:
    id: str
    stream: str
    offset: int
    byte_len: int
    sha256: str
    data_retained: bool

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryRawRecord:
        return cls(
            id=str(value["id"]),
            stream=str(value["stream"]),
            offset=int(value["offset"]),
            byte_len=int(value["byte_len"]),
            sha256=str(value["sha256"]),
            data_retained=bool(value["data_retained"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in self.__dataclass_fields__}


@dataclass(frozen=True, slots=True)
class GeometryLoss:
    code: str
    taxonomy: str
    category: str
    severity: str
    message: str
    stream: str | None
    offset: int | None
    tag: str | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryLoss:
        return cls(
            code=str(value["code"]),
            taxonomy=str(value["taxonomy"]),
            category=str(value["category"]),
            severity=str(value["severity"]),
            message=str(value["message"]),
            stream=_optional_str(value.get("stream")),
            offset=_optional_int(value.get("offset")),
            tag=_optional_str(value.get("tag")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in self.__dataclass_fields__}


@dataclass(frozen=True, slots=True)
class GeometryFinding:
    check: str
    severity: str
    message: str
    entity_id: str | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryFinding:
        return cls(
            check=str(value["check"]),
            severity=str(value["severity"]),
            message=str(value["message"]),
            entity_id=_optional_str(value.get("entity_id")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {name: getattr(self, name) for name in self.__dataclass_fields__}


@dataclass(frozen=True, slots=True)
class GeometryByteDomain:
    id: str
    container_entry_id: str
    stream_path: str
    role: GeometryStreamRole
    storage: GeometryByteStorage
    outer_payload_offset: int
    description: str
    schema: str
    stream_byte_len: int
    stream_sha256: str
    body_offset: int
    byte_len: int
    sha256: str
    offset_basis: GeometryByteOffsetBasis

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryByteDomain:
        return cls(
            id=str(value["id"]),
            container_entry_id=str(value["container_entry_id"]),
            stream_path=str(value["stream_path"]),
            role=GeometryStreamRole(value["role"]),
            storage=GeometryByteStorage(value["storage"]),
            outer_payload_offset=int(value["outer_payload_offset"]),
            description=str(value["description"]),
            schema=str(value["schema"]),
            stream_byte_len=int(value["stream_byte_len"]),
            stream_sha256=str(value["stream_sha256"]),
            body_offset=int(value["body_offset"]),
            byte_len=int(value["byte_len"]),
            sha256=str(value["sha256"]),
            offset_basis=GeometryByteOffsetBasis(value["offset_basis"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "container_entry_id": self.container_entry_id,
            "stream_path": self.stream_path,
            "role": self.role.value,
            "storage": self.storage.value,
            "outer_payload_offset": self.outer_payload_offset,
            "description": self.description,
            "schema": self.schema,
            "stream_byte_len": self.stream_byte_len,
            "stream_sha256": self.stream_sha256,
            "body_offset": self.body_offset,
            "byte_len": self.byte_len,
            "sha256": self.sha256,
            "offset_basis": self.offset_basis.value,
        }


@dataclass(frozen=True, slots=True)
class GeometryByteCoverage:
    source_bytes: int
    candidate_stream_bytes: int
    active_stream_bytes: int
    partition_domain_bytes: int
    retained_record_bytes: int
    located_entity_count: int
    unique_location_count: int
    classified_active_bytes: int
    unclassified_active_bytes: int
    partition_status: GeometryBytePartitionStatus
    typed_bytes: int | None
    uninterpreted_bytes: int | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryByteCoverage:
        return cls(
            source_bytes=int(value["source_bytes"]),
            candidate_stream_bytes=int(value["candidate_stream_bytes"]),
            active_stream_bytes=int(value["active_stream_bytes"]),
            partition_domain_bytes=int(
                value.get("partition_domain_bytes", value["active_stream_bytes"])
            ),
            retained_record_bytes=int(value["retained_record_bytes"]),
            located_entity_count=int(value["located_entity_count"]),
            unique_location_count=int(value["unique_location_count"]),
            classified_active_bytes=int(value["classified_active_bytes"]),
            unclassified_active_bytes=int(value["unclassified_active_bytes"]),
            partition_status=GeometryBytePartitionStatus(value["partition_status"]),
            typed_bytes=_optional_int(value.get("typed_bytes")),
            uninterpreted_bytes=_optional_int(value.get("uninterpreted_bytes")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source_bytes": self.source_bytes,
            "candidate_stream_bytes": self.candidate_stream_bytes,
            "active_stream_bytes": self.active_stream_bytes,
            "partition_domain_bytes": self.partition_domain_bytes,
            "retained_record_bytes": self.retained_record_bytes,
            "located_entity_count": self.located_entity_count,
            "unique_location_count": self.unique_location_count,
            "classified_active_bytes": self.classified_active_bytes,
            "unclassified_active_bytes": self.unclassified_active_bytes,
            "partition_status": self.partition_status.value,
            "typed_bytes": self.typed_bytes,
            "uninterpreted_bytes": self.uninterpreted_bytes,
        }


@dataclass(frozen=True, slots=True)
class GeometryDecodedSpan:
    domain_id: str
    offset: int
    byte_len: int
    classification: GeometryByteClassification
    tag: str
    source_record_id: int | None
    sha256: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryDecodedSpan:
        return cls(
            domain_id=str(value["domain_id"]),
            offset=int(value["offset"]),
            byte_len=int(value["byte_len"]),
            classification=GeometryByteClassification(value["classification"]),
            tag=str(value["tag"]),
            source_record_id=_optional_int(value.get("source_record_id")),
            sha256=str(value["sha256"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {name: getattr(self, name) for name in self.__dataclass_fields__}
        result["classification"] = self.classification.value
        return result


@dataclass(frozen=True, slots=True)
class GeometryByteRange:
    domain_id: str
    offset: int
    byte_len: int
    classification: GeometryByteClassification
    reason: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryByteRange:
        return cls(
            domain_id=str(value["domain_id"]),
            offset=int(value["offset"]),
            byte_len=int(value["byte_len"]),
            classification=GeometryByteClassification(value["classification"]),
            reason=str(value["reason"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {name: getattr(self, name) for name in self.__dataclass_fields__}
        result["classification"] = self.classification.value
        return result


@dataclass(frozen=True, slots=True)
class GeometryFidelityReport:
    decoder: str
    decoder_version: str
    geometry_transferred: bool
    entity_counts: Mapping[str, int]
    byte_domains: tuple[GeometryByteDomain, ...]
    byte_coverage: GeometryByteCoverage
    losses: tuple[GeometryLoss, ...]
    validation_findings: tuple[GeometryFinding, ...]
    byte_spans: tuple[GeometryDecodedSpan, ...] = ()
    byte_ranges: tuple[GeometryByteRange, ...] = ()

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryFidelityReport:
        return cls(
            decoder=str(value["decoder"]),
            decoder_version=str(value["decoder_version"]),
            geometry_transferred=bool(value["geometry_transferred"]),
            entity_counts={
                str(name): int(item)
                for name, item in value.get("entity_counts", {}).items()
            },
            byte_domains=tuple(
                GeometryByteDomain.from_dict(item)
                for item in value.get("byte_domains", [])
            ),
            byte_coverage=GeometryByteCoverage.from_dict(value["byte_coverage"]),
            byte_spans=tuple(
                GeometryDecodedSpan.from_dict(item)
                for item in value.get("byte_spans", [])
            ),
            byte_ranges=tuple(
                GeometryByteRange.from_dict(item)
                for item in value.get("byte_ranges", [])
            ),
            losses=tuple(
                GeometryLoss.from_dict(item) for item in value.get("losses", [])
            ),
            validation_findings=tuple(
                GeometryFinding.from_dict(item)
                for item in value.get("validation_findings", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result = {
            "decoder": self.decoder,
            "decoder_version": self.decoder_version,
            "geometry_transferred": self.geometry_transferred,
            "entity_counts": dict(self.entity_counts),
            "byte_coverage": self.byte_coverage.to_dict(),
            "losses": [item.to_dict() for item in self.losses],
            "validation_findings": [
                item.to_dict() for item in self.validation_findings
            ],
        }
        if self.byte_spans:
            result["byte_spans"] = [item.to_dict() for item in self.byte_spans]
        if self.byte_ranges:
            result["byte_ranges"] = [item.to_dict() for item in self.byte_ranges]
        if self.byte_domains:
            result["byte_domains"] = [item.to_dict() for item in self.byte_domains]
        return result


@dataclass(frozen=True, slots=True)
class GeometryDocument:
    source: SourceInfo
    length_unit: str
    source_streams: tuple[GeometryStreamCandidate, ...]
    model: GeometryModel
    configurations: tuple[GeometryConfigurationState, ...]
    topology_metrics: tuple[GeometryTopologyMetrics, ...]
    raw_records: tuple[GeometryRawRecord, ...]
    fidelity: GeometryFidelityReport

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryDocument:
        return cls(
            source=SourceInfo.from_dict(value["source"]),
            length_unit=str(value["length_unit"]),
            source_streams=tuple(
                GeometryStreamCandidate.from_dict(item)
                for item in value.get("source_streams", [])
            ),
            model=GeometryModel.from_dict(value["model"]),
            configurations=tuple(
                GeometryConfigurationState.from_dict(item)
                for item in value.get("configurations", [])
            ),
            topology_metrics=tuple(
                GeometryTopologyMetrics.from_dict(item)
                for item in value.get("topology_metrics", [])
            ),
            raw_records=tuple(
                GeometryRawRecord.from_dict(item)
                for item in value.get("raw_records", [])
            ),
            fidelity=GeometryFidelityReport.from_dict(value["fidelity"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source": self.source.to_dict(),
            "length_unit": self.length_unit,
            "source_streams": [item.to_dict() for item in self.source_streams],
            "model": self.model.to_dict(),
            "configurations": [item.to_dict() for item in self.configurations],
            "topology_metrics": [item.to_dict() for item in self.topology_metrics],
            "raw_records": [item.to_dict() for item in self.raw_records],
            "fidelity": self.fidelity.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class GeometryResult:
    status: GeometryStatus
    geometry: GeometryDocument | None
    diagnostics: tuple[Diagnostic, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryResult:
        geometry = value.get("geometry")
        return cls(
            status=GeometryStatus(value["status"]),
            geometry=(
                None if geometry is None else GeometryDocument.from_dict(geometry)
            ),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "geometry": None if self.geometry is None else self.geometry.to_dict(),
            "diagnostics": [item.to_dict() for item in self.diagnostics],
        }


class DisplayMeshStatus(_StringEnum):
    DECODED = "decoded"
    PARTIAL = "partial"
    UNSUPPORTED = "unsupported"
    MALFORMED = "malformed"
    REJECTED = "rejected"


class DisplayColorSource(_StringEnum):
    BODY = "body"
    FEATURE = "feature"
    FACE = "face"


@dataclass(frozen=True, slots=True)
class DisplaySurfaceReference:
    identity: str
    feature_source_id: int
    local_surface_id: int

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DisplaySurfaceReference:
        return cls(
            identity=str(value["identity"]),
            feature_source_id=int(value["feature_source_id"]),
            local_surface_id=int(value["local_surface_id"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "identity": self.identity,
            "feature_source_id": self.feature_source_id,
            "local_surface_id": self.local_surface_id,
        }


@dataclass(frozen=True, slots=True)
class DisplayMeshPart:
    """One face table and its slices of the packed buffers."""

    id: str
    stream_index: int
    table_index: int
    table_offset: int
    vertex_start: int
    vertex_count: int
    triangle_start: int
    triangle_count: int
    edge_start: int
    edge_count: int
    strip_count: int
    normals_derived: bool
    color: tuple[float, float, float, float] | None
    color_source: DisplayColorSource | None
    appearance_name: str | None
    surface_references: tuple[DisplaySurfaceReference, ...] = ()

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DisplayMeshPart:
        source = value.get("color_source")
        return cls(
            id=str(value["id"]),
            stream_index=int(value["stream_index"]),
            table_index=int(value["table_index"]),
            table_offset=int(value["table_offset"]),
            vertex_start=int(value["vertex_start"]),
            vertex_count=int(value["vertex_count"]),
            triangle_start=int(value["triangle_start"]),
            triangle_count=int(value["triangle_count"]),
            edge_start=int(value["edge_start"]),
            edge_count=int(value["edge_count"]),
            strip_count=int(value["strip_count"]),
            normals_derived=bool(value["normals_derived"]),
            color=_optional_float_quad(value.get("color")),
            color_source=None if source is None else DisplayColorSource(source),
            appearance_name=_optional_str(value.get("appearance_name")),
            surface_references=tuple(
                DisplaySurfaceReference.from_dict(item)
                for item in value.get("surface_references", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "stream_index": self.stream_index,
            "table_index": self.table_index,
            "table_offset": self.table_offset,
            "vertex_start": self.vertex_start,
            "vertex_count": self.vertex_count,
            "triangle_start": self.triangle_start,
            "triangle_count": self.triangle_count,
            "edge_start": self.edge_start,
            "edge_count": self.edge_count,
            "strip_count": self.strip_count,
            "normals_derived": self.normals_derived,
            "color": None if self.color is None else list(self.color),
            "color_source": (
                None if self.color_source is None else self.color_source.value
            ),
            "appearance_name": self.appearance_name,
        }
        if self.surface_references:
            result["surface_references"] = [
                item.to_dict() for item in self.surface_references
            ]
        return result


@dataclass(frozen=True, slots=True)
class DisplayMeshStream:
    entry_id: str
    stream_path: str
    decoded_size: int
    decoded_sha256: str | None
    part_count: int
    trailing_names: tuple[str, ...] = ()

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DisplayMeshStream:
        return cls(
            entry_id=str(value["entry_id"]),
            stream_path=str(value["stream_path"]),
            decoded_size=int(value["decoded_size"]),
            decoded_sha256=_optional_str(value.get("decoded_sha256")),
            part_count=int(value["part_count"]),
            trailing_names=_string_tuple(value.get("trailing_names", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "entry_id": self.entry_id,
            "stream_path": self.stream_path,
            "decoded_size": self.decoded_size,
            "decoded_sha256": self.decoded_sha256,
            "part_count": self.part_count,
        }
        if self.trailing_names:
            result["trailing_names"] = list(self.trailing_names)
        return result


@dataclass(frozen=True, slots=True)
class DisplayMeshBounds:
    min: tuple[float, float, float]
    max: tuple[float, float, float]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DisplayMeshBounds:
        return cls(min=_float_triple(value["min"]), max=_float_triple(value["max"]))

    def to_dict(self) -> dict[str, Any]:
        return {"min": list(self.min), "max": list(self.max)}


@dataclass(frozen=True, slots=True)
class DisplayMeshDocument:
    source: SourceInfo
    envelope: Envelope
    document_kind: DocumentKind
    internal_version: int | None
    length_unit: str
    streams: tuple[DisplayMeshStream, ...]
    parts: tuple[DisplayMeshPart, ...]
    vertex_count: int
    triangle_count: int
    edge_count: int
    bounds: DisplayMeshBounds | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DisplayMeshDocument:
        bounds = value.get("bounds")
        return cls(
            source=SourceInfo.from_dict(value["source"]),
            envelope=Envelope(value["envelope"]),
            document_kind=DocumentKind(value["document_kind"]),
            internal_version=_optional_int(value.get("internal_version")),
            length_unit=str(value["length_unit"]),
            streams=tuple(
                DisplayMeshStream.from_dict(item) for item in value.get("streams", [])
            ),
            parts=tuple(
                DisplayMeshPart.from_dict(item) for item in value.get("parts", [])
            ),
            vertex_count=int(value["vertex_count"]),
            triangle_count=int(value["triangle_count"]),
            edge_count=int(value["edge_count"]),
            bounds=None if bounds is None else DisplayMeshBounds.from_dict(bounds),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source": self.source.to_dict(),
            "envelope": self.envelope.value,
            "document_kind": self.document_kind.value,
            "internal_version": self.internal_version,
            "length_unit": self.length_unit,
            "streams": [item.to_dict() for item in self.streams],
            "parts": [item.to_dict() for item in self.parts],
            "vertex_count": self.vertex_count,
            "triangle_count": self.triangle_count,
            "edge_count": self.edge_count,
            "bounds": None if self.bounds is None else self.bounds.to_dict(),
        }


@dataclass(frozen=True, slots=True)
class DisplayMeshResult:
    """Saved display meshes plus packed little-endian buffers.

    ``positions`` and ``normals`` hold three ``float32`` values per vertex
    (positions in millimetres). ``indices`` holds three ``uint32`` vertex
    indices per triangle and ``edges`` two per face-outline segment. The
    buffers are not part of ``to_dict()``.
    """

    status: DisplayMeshStatus
    mesh: DisplayMeshDocument | None
    diagnostics: tuple[Diagnostic, ...]
    positions: bytes = b""
    normals: bytes = b""
    indices: bytes = b""
    edges: bytes = b""

    @classmethod
    def from_dict(
        cls,
        value: Mapping[str, Any],
        *,
        positions: bytes = b"",
        normals: bytes = b"",
        indices: bytes = b"",
        edges: bytes = b"",
    ) -> DisplayMeshResult:
        mesh = value.get("mesh")
        return cls(
            status=DisplayMeshStatus(value["status"]),
            mesh=None if mesh is None else DisplayMeshDocument.from_dict(mesh),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
            positions=bytes(positions),
            normals=bytes(normals),
            indices=bytes(indices),
            edges=bytes(edges),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "mesh": None if self.mesh is None else self.mesh.to_dict(),
            "diagnostics": [item.to_dict() for item in self.diagnostics],
        }


@dataclass(frozen=True, slots=True)
class GeometryEvaluationOptions:
    chord_tolerance_mm: float = 0.01
    max_segments_per_edge: int = 512
    tangent_angle_deg: float = 0.5
    mesh_feature_angle_deg: float = 30.0
    configuration: str | None = None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryEvaluationOptions:
        return cls(
            chord_tolerance_mm=float(value.get("chord_tolerance_mm", 0.01)),
            max_segments_per_edge=int(value.get("max_segments_per_edge", 512)),
            tangent_angle_deg=float(value.get("tangent_angle_deg", 0.5)),
            mesh_feature_angle_deg=float(value.get("mesh_feature_angle_deg", 30.0)),
            configuration=_optional_str(value.get("configuration")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "chord_tolerance_mm": self.chord_tolerance_mm,
            "max_segments_per_edge": self.max_segments_per_edge,
            "tangent_angle_deg": self.tangent_angle_deg,
            "mesh_feature_angle_deg": self.mesh_feature_angle_deg,
            "configuration": self.configuration,
        }


@dataclass(frozen=True, slots=True)
class EvaluatedBounds:
    min: tuple[float, float, float]
    max: tuple[float, float, float]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> EvaluatedBounds:
        return cls(min=_float_triple(value["min"]), max=_float_triple(value["max"]))

    def to_dict(self) -> dict[str, Any]:
        return {"min": list(self.min), "max": list(self.max)}

    @property
    def size(self) -> tuple[float, float, float]:
        """Extent along each axis in millimetres."""
        return (
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        )


@dataclass(frozen=True, slots=True)
class EvaluatedBoundingBox:
    bounds: EvaluatedBounds
    method: BoundingBoxMethod
    exact: bool
    edge_count: int
    vertex_count: int

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> EvaluatedBoundingBox:
        return cls(
            bounds=EvaluatedBounds.from_dict(value["bounds"]),
            method=BoundingBoxMethod(value["method"]),
            exact=bool(value["exact"]),
            edge_count=int(value["edge_count"]),
            vertex_count=int(value["vertex_count"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "bounds": self.bounds.to_dict(),
            "method": self.method.value,
            "exact": self.exact,
            "edge_count": self.edge_count,
            "vertex_count": self.vertex_count,
        }


@dataclass(frozen=True, slots=True)
class EvaluatedEdge:
    edge_id: str
    curve_id: str | None
    curve_kind: str
    body_id: str | None
    interval_source: EdgeIntervalSource
    parameter_range: tuple[float, float] | None
    status: EdgeSampleStatus
    reason: str | None
    polyline: tuple[tuple[float, float, float], ...]
    segment_count: int
    chord_tolerance_mm: float | None
    adjacent_face_ids: tuple[str, ...]
    smoothness: EdgeSmoothness
    dihedral_angle_deg: float | None
    is_derived_seam: bool

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> EvaluatedEdge:
        return cls(
            edge_id=str(value["edge_id"]),
            curve_id=_optional_str(value.get("curve_id")),
            curve_kind=str(value["curve_kind"]),
            body_id=_optional_str(value.get("body_id")),
            interval_source=EdgeIntervalSource(value["interval_source"]),
            parameter_range=_optional_float_pair(value.get("parameter_range")),
            status=EdgeSampleStatus(value["status"]),
            reason=_optional_str(value.get("reason")),
            polyline=tuple(_float_triple(item) for item in value.get("polyline", [])),
            segment_count=int(value["segment_count"]),
            chord_tolerance_mm=_optional_float(value.get("chord_tolerance_mm")),
            adjacent_face_ids=tuple(
                str(item) for item in value.get("adjacent_face_ids", [])
            ),
            smoothness=EdgeSmoothness(value["smoothness"]),
            dihedral_angle_deg=_optional_float(value.get("dihedral_angle_deg")),
            is_derived_seam=bool(value["is_derived_seam"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "edge_id": self.edge_id,
            "curve_id": self.curve_id,
            "curve_kind": self.curve_kind,
            "body_id": self.body_id,
            "interval_source": self.interval_source.value,
            "parameter_range": (
                None if self.parameter_range is None else list(self.parameter_range)
            ),
            "status": self.status.value,
            "reason": self.reason,
            "segment_count": self.segment_count,
            "chord_tolerance_mm": self.chord_tolerance_mm,
            "smoothness": self.smoothness.value,
            "dihedral_angle_deg": self.dihedral_angle_deg,
            "is_derived_seam": self.is_derived_seam,
        }
        if self.polyline:
            result["polyline"] = [list(point) for point in self.polyline]
        if self.adjacent_face_ids:
            result["adjacent_face_ids"] = list(self.adjacent_face_ids)
        return result


@dataclass(frozen=True, slots=True)
class EvaluatedBodyBounds:
    body_id: str
    bounding_box: EvaluatedBoundingBox | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> EvaluatedBodyBounds:
        box = value.get("bounding_box")
        return cls(
            body_id=str(value["body_id"]),
            bounding_box=None if box is None else EvaluatedBoundingBox.from_dict(box),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "body_id": self.body_id,
            "bounding_box": (
                None if self.bounding_box is None else self.bounding_box.to_dict()
            ),
        }


@dataclass(frozen=True, slots=True)
class EvaluatedConfigurationBounds:
    configuration_id: str
    name: str | None
    body_ids: tuple[str, ...] | None
    bounding_box: EvaluatedBoundingBox | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> EvaluatedConfigurationBounds:
        body_ids = value.get("body_ids")
        box = value.get("bounding_box")
        return cls(
            configuration_id=str(value["configuration_id"]),
            name=_optional_str(value.get("name")),
            body_ids=None
            if body_ids is None
            else tuple(str(item) for item in body_ids),
            bounding_box=None if box is None else EvaluatedBoundingBox.from_dict(box),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "configuration_id": self.configuration_id,
            "name": self.name,
            "body_ids": None if self.body_ids is None else list(self.body_ids),
            "bounding_box": (
                None if self.bounding_box is None else self.bounding_box.to_dict()
            ),
        }


@dataclass(frozen=True, slots=True)
class EvaluatedMesh:
    tessellation_id: str
    body_id: str | None
    vertex_count: int
    triangle_count: int
    bounds: EvaluatedBounds | None
    area_mm2: float
    is_closed: bool | None
    signed_volume_mm3: float | None
    feature_edges: tuple[tuple[int, int], ...]
    boundary_edges: tuple[tuple[int, int], ...]
    feature_angle_deg: float

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> EvaluatedMesh:
        bounds = value.get("bounds")
        return cls(
            tessellation_id=str(value["tessellation_id"]),
            body_id=_optional_str(value.get("body_id")),
            vertex_count=int(value["vertex_count"]),
            triangle_count=int(value["triangle_count"]),
            bounds=None if bounds is None else EvaluatedBounds.from_dict(bounds),
            area_mm2=float(value["area_mm2"]),
            is_closed=_optional_bool(value.get("is_closed")),
            signed_volume_mm3=_optional_float(value.get("signed_volume_mm3")),
            feature_edges=tuple(
                _int_pair(item) for item in value.get("feature_edges", [])
            ),
            boundary_edges=tuple(
                _int_pair(item) for item in value.get("boundary_edges", [])
            ),
            feature_angle_deg=float(value["feature_angle_deg"]),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "tessellation_id": self.tessellation_id,
            "body_id": self.body_id,
            "vertex_count": self.vertex_count,
            "triangle_count": self.triangle_count,
            "bounds": None if self.bounds is None else self.bounds.to_dict(),
            "area_mm2": self.area_mm2,
            "is_closed": self.is_closed,
            "signed_volume_mm3": self.signed_volume_mm3,
            "feature_angle_deg": self.feature_angle_deg,
        }
        if self.feature_edges:
            result["feature_edges"] = [list(item) for item in self.feature_edges]
        if self.boundary_edges:
            result["boundary_edges"] = [list(item) for item in self.boundary_edges]
        return result


@dataclass(frozen=True, slots=True)
class GeometryEvaluationSummary:
    edge_count: int
    sampled_edge_count: int
    unsampled_edge_count: int
    sharp_edge_count: int
    tangent_edge_count: int
    boundary_edge_count: int
    unknown_smoothness_count: int
    mesh_count: int
    closed_mesh_count: int

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryEvaluationSummary:
        return cls(**{name: int(value[name]) for name in cls.__dataclass_fields__})

    def to_dict(self) -> dict[str, int]:
        return {name: getattr(self, name) for name in self.__dataclass_fields__}


@dataclass(frozen=True, slots=True)
class GeometryEvaluationDocument:
    source: SourceInfo
    length_unit: str
    options: GeometryEvaluationOptions
    bounding_box: EvaluatedBoundingBox | None
    body_bounds: tuple[EvaluatedBodyBounds, ...]
    configuration_bounds: tuple[EvaluatedConfigurationBounds, ...]
    edges: tuple[EvaluatedEdge, ...]
    meshes: tuple[EvaluatedMesh, ...]
    summary: GeometryEvaluationSummary
    losses: tuple[GeometryLoss, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryEvaluationDocument:
        box = value.get("bounding_box")
        return cls(
            source=SourceInfo.from_dict(value["source"]),
            length_unit=str(value["length_unit"]),
            options=GeometryEvaluationOptions.from_dict(value["options"]),
            bounding_box=None if box is None else EvaluatedBoundingBox.from_dict(box),
            body_bounds=tuple(
                EvaluatedBodyBounds.from_dict(item)
                for item in value.get("body_bounds", [])
            ),
            configuration_bounds=tuple(
                EvaluatedConfigurationBounds.from_dict(item)
                for item in value.get("configuration_bounds", [])
            ),
            edges=tuple(
                EvaluatedEdge.from_dict(item) for item in value.get("edges", [])
            ),
            meshes=tuple(
                EvaluatedMesh.from_dict(item) for item in value.get("meshes", [])
            ),
            summary=GeometryEvaluationSummary.from_dict(value["summary"]),
            losses=tuple(
                GeometryLoss.from_dict(item) for item in value.get("losses", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source": self.source.to_dict(),
            "length_unit": self.length_unit,
            "options": self.options.to_dict(),
            "bounding_box": (
                None if self.bounding_box is None else self.bounding_box.to_dict()
            ),
            "body_bounds": [item.to_dict() for item in self.body_bounds],
            "configuration_bounds": [
                item.to_dict() for item in self.configuration_bounds
            ],
            "edges": [item.to_dict() for item in self.edges],
            "meshes": [item.to_dict() for item in self.meshes],
            "summary": self.summary.to_dict(),
            "losses": [item.to_dict() for item in self.losses],
        }

    def sampled_edges(self) -> tuple[EvaluatedEdge, ...]:
        """Return the edges that have a polyline."""
        return tuple(
            edge for edge in self.edges if edge.status is EdgeSampleStatus.SAMPLED
        )


@dataclass(frozen=True, slots=True)
class GeometryEvaluationResult:
    status: GeometryEvaluationStatus
    evaluation: GeometryEvaluationDocument | None
    diagnostics: tuple[Diagnostic, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryEvaluationResult:
        evaluation = value.get("evaluation")
        return cls(
            status=GeometryEvaluationStatus(value["status"]),
            evaluation=(
                None
                if evaluation is None
                else GeometryEvaluationDocument.from_dict(evaluation)
            ),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "evaluation": (
                None if self.evaluation is None else self.evaluation.to_dict()
            ),
            "diagnostics": [item.to_dict() for item in self.diagnostics],
        }


@dataclass(frozen=True, slots=True)
class SourceDocument:
    source: SourceInfo
    envelope: SourcedValue[Envelope]
    document_kind: SourcedValue[DocumentKind]
    internal_version: SourcedValue[int] | None
    configurations: tuple[Configuration, ...]
    properties: tuple[CustomProperty, ...]
    references: tuple[DocumentReference, ...]
    preview: BinaryResource | None
    sheets: tuple[DrawingSheet, ...]
    unknown_records: tuple[UnknownRecord, ...]
    units: DocumentUnits | None = None
    feature_records: tuple[FeatureRecord, ...] = ()
    embedded_documents: tuple[EmbeddedDocument, ...] = ()

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> SourceDocument:
        internal_version = value.get("internal_version")
        preview = value.get("preview")
        units = value.get("units")
        return cls(
            source=SourceInfo.from_dict(value["source"]),
            envelope=SourcedValue.from_dict(value["envelope"], Envelope),
            document_kind=SourcedValue.from_dict(value["document_kind"], DocumentKind),
            internal_version=(
                None
                if internal_version is None
                else SourcedValue.from_dict(internal_version, int)
            ),
            configurations=tuple(
                Configuration.from_dict(item)
                for item in value.get("configurations", [])
            ),
            properties=tuple(
                CustomProperty.from_dict(item) for item in value.get("properties", [])
            ),
            references=tuple(
                DocumentReference.from_dict(item)
                for item in value.get("references", [])
            ),
            preview=(None if preview is None else BinaryResource.from_dict(preview)),
            sheets=tuple(
                DrawingSheet.from_dict(item) for item in value.get("sheets", [])
            ),
            unknown_records=tuple(
                UnknownRecord.from_dict(item)
                for item in value.get("unknown_records", [])
            ),
            units=None if units is None else DocumentUnits.from_dict(units),
            feature_records=tuple(
                FeatureRecord.from_dict(item)
                for item in value.get("feature_records", [])
            ),
            embedded_documents=tuple(
                EmbeddedDocument.from_dict(item)
                for item in value.get("embedded_documents", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "source": self.source.to_dict(),
            "envelope": self.envelope.to_dict(),
            "document_kind": self.document_kind.to_dict(),
            "internal_version": (
                None
                if self.internal_version is None
                else self.internal_version.to_dict()
            ),
            "configurations": [item.to_dict() for item in self.configurations],
            "properties": [item.to_dict() for item in self.properties],
            "references": [item.to_dict() for item in self.references],
            "preview": None if self.preview is None else self.preview.to_dict(),
            "sheets": [item.to_dict() for item in self.sheets],
            "unknown_records": [item.to_dict() for item in self.unknown_records],
        }
        if self.units is not None:
            result["units"] = self.units.to_dict()
        if self.feature_records:
            result["feature_records"] = [
                item.to_dict() for item in self.feature_records
            ]
        if self.embedded_documents:
            result["embedded_documents"] = [
                item.to_dict() for item in self.embedded_documents
            ]
        return result


@dataclass(frozen=True, slots=True)
class SemanticCoverage:
    decoded_streams_total: int
    fully_interpreted_streams: int
    partially_interpreted_streams: int
    uninterpreted_streams: int
    malformed_streams: int
    decoded_bytes_total: int
    fully_interpreted_bytes: int
    partially_interpreted_bytes: int
    uninterpreted_bytes: int
    malformed_bytes: int

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> SemanticCoverage:
        return cls(**{name: int(value[name]) for name in cls.__dataclass_fields__})

    def to_dict(self) -> dict[str, int]:
        return {name: int(getattr(self, name)) for name in self.__dataclass_fields__}


@dataclass(frozen=True, slots=True)
class ParseResult:
    status: ParseStatus
    document: SourceDocument | None
    inventory: ContainerInventory | None
    diagnostics: tuple[Diagnostic, ...]
    coverage: CoverageReport
    semantic_coverage: SemanticCoverage | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ParseResult:
        document = value.get("document")
        inventory = value.get("inventory")
        semantic_coverage = value.get("semantic_coverage")
        return cls(
            status=ParseStatus(value["status"]),
            document=None if document is None else SourceDocument.from_dict(document),
            inventory=(
                None if inventory is None else ContainerInventory.from_dict(inventory)
            ),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
            coverage=CoverageReport.from_dict(value["coverage"]),
            semantic_coverage=(
                None
                if semantic_coverage is None
                else SemanticCoverage.from_dict(semantic_coverage)
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "status": self.status.value,
            "document": None if self.document is None else self.document.to_dict(),
            "inventory": (None if self.inventory is None else self.inventory.to_dict()),
            "diagnostics": [item.to_dict() for item in self.diagnostics],
            "coverage": self.coverage.to_dict(),
            "semantic_coverage": (
                None
                if self.semantic_coverage is None
                else self.semantic_coverage.to_dict()
            ),
        }


@dataclass(frozen=True, slots=True)
class ProjectConfigurationIdentity:
    index: int
    name: str | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ProjectConfigurationIdentity:
        return cls(index=int(value["index"]), name=_optional_str(value.get("name")))

    def to_dict(self) -> dict[str, Any]:
        return {"index": self.index, "name": self.name}


@dataclass(frozen=True, slots=True)
class EmbeddedNodeSource:
    container_node_id: str
    entry_id: str
    stream_path: str

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> EmbeddedNodeSource:
        return cls(
            container_node_id=str(value["container_node_id"]),
            entry_id=str(value["entry_id"]),
            stream_path=str(value["stream_path"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "container_node_id": self.container_node_id,
            "entry_id": self.entry_id,
            "stream_path": self.stream_path,
        }


@dataclass(frozen=True, slots=True)
class ProjectNode:
    id: str
    path: str
    is_root: bool
    minimum_depth: int
    parse_status: ParseStatus | None
    document_kind: DocumentKind | None
    byte_len: int | None
    source_sha256: str | None
    available_configurations: tuple[ProjectConfigurationIdentity, ...]
    selected_configuration_indices: tuple[int, ...]
    requested_configurations: tuple[str, ...]
    diagnostic_codes: tuple[str, ...]
    embedded_source: EmbeddedNodeSource | None = None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ProjectNode:
        parse_status = value.get("parse_status")
        document_kind = value.get("document_kind")
        embedded_source = value.get("embedded_source")
        return cls(
            id=str(value["id"]),
            path=str(value["path"]),
            is_root=bool(value["is_root"]),
            minimum_depth=int(value["minimum_depth"]),
            parse_status=(None if parse_status is None else ParseStatus(parse_status)),
            document_kind=(
                None if document_kind is None else DocumentKind(document_kind)
            ),
            byte_len=_optional_int(value.get("byte_len")),
            source_sha256=_optional_str(value.get("source_sha256")),
            available_configurations=tuple(
                ProjectConfigurationIdentity.from_dict(item)
                for item in value.get("available_configurations", [])
            ),
            selected_configuration_indices=tuple(
                int(item) for item in value.get("selected_configuration_indices", [])
            ),
            requested_configurations=tuple(
                str(item) for item in value.get("requested_configurations", [])
            ),
            diagnostic_codes=tuple(
                str(item) for item in value.get("diagnostic_codes", [])
            ),
            embedded_source=(
                None
                if embedded_source is None
                else EmbeddedNodeSource.from_dict(embedded_source)
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "path": self.path,
            "is_root": self.is_root,
            "minimum_depth": self.minimum_depth,
            "parse_status": (
                None if self.parse_status is None else self.parse_status.value
            ),
            "document_kind": (
                None if self.document_kind is None else self.document_kind.value
            ),
            "byte_len": self.byte_len,
            "source_sha256": self.source_sha256,
            "available_configurations": [
                item.to_dict() for item in self.available_configurations
            ],
            "selected_configuration_indices": list(self.selected_configuration_indices),
            "requested_configurations": list(self.requested_configurations),
            "diagnostic_codes": list(self.diagnostic_codes),
        }
        if self.embedded_source is not None:
            result["embedded_source"] = self.embedded_source.to_dict()
        return result


@dataclass(frozen=True, slots=True)
class ProjectEdge:
    id: str
    source_node_id: str
    reference_index: int
    kind: ReferenceKind
    source_configuration_index: int | None
    source_name: str | None
    stored_path: str | None
    referenced_configuration: str | None
    expected_document_kind: DocumentKind | None
    is_suppressed: bool | None
    is_hidden: bool | None
    exclude_from_bom: bool | None
    resolution_status: ReferenceResolutionStatus
    resolution_basis: ReferenceResolutionBasis | None
    candidate_count: int
    candidate_paths: tuple[str, ...]
    resolved_path: str | None
    target_node_id: str | None
    traversal_status: ReferenceTraversalStatus
    is_virtual: bool | None = None
    instance_number: int | None = None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ProjectEdge:
        expected_kind = value.get("expected_document_kind")
        resolution_basis = value.get("resolution_basis")
        return cls(
            id=str(value["id"]),
            source_node_id=str(value["source_node_id"]),
            reference_index=int(value["reference_index"]),
            kind=ReferenceKind(value["kind"]),
            source_configuration_index=_optional_int(
                value.get("source_configuration_index")
            ),
            source_name=_optional_str(value.get("source_name")),
            stored_path=_optional_str(value.get("stored_path")),
            referenced_configuration=_optional_str(
                value.get("referenced_configuration")
            ),
            expected_document_kind=(
                None if expected_kind is None else DocumentKind(expected_kind)
            ),
            is_suppressed=_optional_bool(value.get("is_suppressed")),
            is_hidden=_optional_bool(value.get("is_hidden")),
            exclude_from_bom=_optional_bool(value.get("exclude_from_bom")),
            resolution_status=ReferenceResolutionStatus(value["resolution_status"]),
            resolution_basis=(
                None
                if resolution_basis is None
                else ReferenceResolutionBasis(resolution_basis)
            ),
            candidate_count=int(value["candidate_count"]),
            candidate_paths=tuple(
                str(item) for item in value.get("candidate_paths", [])
            ),
            resolved_path=_optional_str(value.get("resolved_path")),
            target_node_id=_optional_str(value.get("target_node_id")),
            traversal_status=ReferenceTraversalStatus(value["traversal_status"]),
            is_virtual=_optional_bool(value.get("is_virtual")),
            instance_number=_optional_int(value.get("instance_number")),
        )

    def to_dict(self) -> dict[str, Any]:
        result: dict[str, Any] = {
            "id": self.id,
            "source_node_id": self.source_node_id,
            "reference_index": self.reference_index,
            "kind": self.kind.value,
            "source_configuration_index": self.source_configuration_index,
            "source_name": self.source_name,
            "stored_path": self.stored_path,
            "referenced_configuration": self.referenced_configuration,
            "expected_document_kind": (
                None
                if self.expected_document_kind is None
                else self.expected_document_kind.value
            ),
            "is_suppressed": self.is_suppressed,
            "is_hidden": self.is_hidden,
            "exclude_from_bom": self.exclude_from_bom,
            "resolution_status": self.resolution_status.value,
            "resolution_basis": (
                None if self.resolution_basis is None else self.resolution_basis.value
            ),
            "candidate_count": self.candidate_count,
            "candidate_paths": list(self.candidate_paths),
            "resolved_path": self.resolved_path,
            "target_node_id": self.target_node_id,
            "traversal_status": self.traversal_status.value,
        }
        if self.is_virtual is not None:
            result["is_virtual"] = self.is_virtual
        if self.instance_number is not None:
            result["instance_number"] = self.instance_number
        return result


@dataclass(frozen=True, slots=True)
class ProjectCompatibilityReport:
    schema_version: int
    status: ProjectScanStatus
    node_count: int
    scanned_node_count: int
    edge_count: int
    document_kind_counts: Mapping[str, int]
    parse_status_counts: Mapping[str, int]
    resolution_status_counts: Mapping[str, int]
    traversal_status_counts: Mapping[str, int]
    diagnostic_code_counts: Mapping[str, int]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ProjectCompatibilityReport:
        return cls(
            schema_version=int(value["schema_version"]),
            status=ProjectScanStatus(value["status"]),
            node_count=int(value["node_count"]),
            scanned_node_count=int(value["scanned_node_count"]),
            edge_count=int(value["edge_count"]),
            document_kind_counts=_int_mapping(value.get("document_kind_counts", {})),
            parse_status_counts=_int_mapping(value.get("parse_status_counts", {})),
            resolution_status_counts=_int_mapping(
                value.get("resolution_status_counts", {})
            ),
            traversal_status_counts=_int_mapping(
                value.get("traversal_status_counts", {})
            ),
            diagnostic_code_counts=_int_mapping(
                value.get("diagnostic_code_counts", {})
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "status": self.status.value,
            "node_count": self.node_count,
            "scanned_node_count": self.scanned_node_count,
            "edge_count": self.edge_count,
            "document_kind_counts": dict(self.document_kind_counts),
            "parse_status_counts": dict(self.parse_status_counts),
            "resolution_status_counts": dict(self.resolution_status_counts),
            "traversal_status_counts": dict(self.traversal_status_counts),
            "diagnostic_code_counts": dict(self.diagnostic_code_counts),
        }


@dataclass(frozen=True, slots=True)
class ProjectScanResult:
    schema_version: int
    status: ProjectScanStatus
    root_node_id: str | None
    nodes: tuple[ProjectNode, ...]
    edges: tuple[ProjectEdge, ...]
    diagnostics: tuple[Diagnostic, ...]
    compatibility_report: ProjectCompatibilityReport

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ProjectScanResult:
        return cls(
            schema_version=int(value["schema_version"]),
            status=ProjectScanStatus(value["status"]),
            root_node_id=_optional_str(value.get("root_node_id")),
            nodes=tuple(ProjectNode.from_dict(item) for item in value.get("nodes", [])),
            edges=tuple(ProjectEdge.from_dict(item) for item in value.get("edges", [])),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
            compatibility_report=ProjectCompatibilityReport.from_dict(
                value["compatibility_report"]
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "status": self.status.value,
            "root_node_id": self.root_node_id,
            "nodes": [item.to_dict() for item in self.nodes],
            "edges": [item.to_dict() for item in self.edges],
            "diagnostics": [item.to_dict() for item in self.diagnostics],
            "compatibility_report": self.compatibility_report.to_dict(),
        }

    def node_by_path(self, path: str) -> ProjectNode | None:
        """Return the node whose project-relative label equals ``path``."""
        for node in self.nodes:
            if node.path == path:
                return node
        return None

    def edges_to(self, node_id: str) -> tuple[ProjectEdge, ...]:
        """Return every edge that resolved to ``node_id`` (reverse lookup)."""
        return tuple(edge for edge in self.edges if edge.target_node_id == node_id)

    def referencing_nodes(
        self, node_id: str, *, kind: ReferenceKind | None = None
    ) -> tuple[ProjectNode, ...]:
        """Return the distinct nodes whose references resolve to ``node_id``."""
        by_id = {node.id: node for node in self.nodes}
        seen: dict[str, ProjectNode] = {}
        for edge in self.edges_to(node_id):
            if kind is not None and edge.kind is not kind:
                continue
            source = by_id.get(edge.source_node_id)
            if source is not None:
                seen.setdefault(source.id, source)
        return tuple(seen.values())


@dataclass(frozen=True, slots=True)
class BomOptions:
    root_configuration: str | None = None
    include_suppressed: bool = False
    include_excluded: bool = False
    max_occurrences: int = 1_000_000

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> BomOptions:
        return cls(
            root_configuration=_optional_str(value.get("root_configuration")),
            include_suppressed=bool(value.get("include_suppressed", False)),
            include_excluded=bool(value.get("include_excluded", False)),
            max_occurrences=int(value.get("max_occurrences", 1_000_000)),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "root_configuration": self.root_configuration,
            "include_suppressed": self.include_suppressed,
            "include_excluded": self.include_excluded,
            "max_occurrences": self.max_occurrences,
        }


@dataclass(frozen=True, slots=True)
class BomOccurrence:
    id: str
    parent_id: str | None
    edge_id: str | None
    node_id: str | None
    depth: int
    instance_name: str | None
    instance_number: int | None
    stored_path: str | None
    document_kind: DocumentKind | None
    referenced_configuration: str | None
    configuration_index: int | None
    is_suppressed: bool | None
    is_hidden: bool | None
    exclude_from_bom: bool | None
    is_virtual: bool | None
    resolution_status: ReferenceResolutionStatus | None
    traversal_status: ReferenceTraversalStatus | None
    children_state: BomChildrenState
    counted: bool

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> BomOccurrence:
        document_kind = value.get("document_kind")
        resolution = value.get("resolution_status")
        traversal = value.get("traversal_status")
        return cls(
            id=str(value["id"]),
            parent_id=_optional_str(value.get("parent_id")),
            edge_id=_optional_str(value.get("edge_id")),
            node_id=_optional_str(value.get("node_id")),
            depth=int(value["depth"]),
            instance_name=_optional_str(value.get("instance_name")),
            instance_number=_optional_int(value.get("instance_number")),
            stored_path=_optional_str(value.get("stored_path")),
            document_kind=(
                None if document_kind is None else DocumentKind(document_kind)
            ),
            referenced_configuration=_optional_str(
                value.get("referenced_configuration")
            ),
            configuration_index=_optional_int(value.get("configuration_index")),
            is_suppressed=_optional_bool(value.get("is_suppressed")),
            is_hidden=_optional_bool(value.get("is_hidden")),
            exclude_from_bom=_optional_bool(value.get("exclude_from_bom")),
            is_virtual=_optional_bool(value.get("is_virtual")),
            resolution_status=(
                None if resolution is None else ReferenceResolutionStatus(resolution)
            ),
            traversal_status=(
                None if traversal is None else ReferenceTraversalStatus(traversal)
            ),
            children_state=BomChildrenState(value["children_state"]),
            counted=bool(value["counted"]),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "parent_id": self.parent_id,
            "edge_id": self.edge_id,
            "node_id": self.node_id,
            "depth": self.depth,
            "instance_name": self.instance_name,
            "instance_number": self.instance_number,
            "stored_path": self.stored_path,
            "document_kind": (
                None if self.document_kind is None else self.document_kind.value
            ),
            "referenced_configuration": self.referenced_configuration,
            "configuration_index": self.configuration_index,
            "is_suppressed": self.is_suppressed,
            "is_hidden": self.is_hidden,
            "exclude_from_bom": self.exclude_from_bom,
            "is_virtual": self.is_virtual,
            "resolution_status": (
                None if self.resolution_status is None else self.resolution_status.value
            ),
            "traversal_status": (
                None if self.traversal_status is None else self.traversal_status.value
            ),
            "children_state": self.children_state.value,
            "counted": self.counted,
        }


@dataclass(frozen=True, slots=True)
class BomItemKey:
    node_id: str | None
    stored_basename: str | None
    configuration: str | None

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> BomItemKey:
        return cls(
            node_id=_optional_str(value.get("node_id")),
            stored_basename=_optional_str(value.get("stored_basename")),
            configuration=_optional_str(value.get("configuration")),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "node_id": self.node_id,
            "stored_basename": self.stored_basename,
            "configuration": self.configuration,
        }


@dataclass(frozen=True, slots=True)
class BomItem:
    key: BomItemKey
    document_path: str | None
    document_kind: DocumentKind | None
    display_name: str | None
    quantity: int
    suppressed_quantity: int
    excluded_quantity: int
    unknown_state_quantity: int
    minimum_depth: int
    is_virtual: bool | None
    occurrence_ids: tuple[str, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> BomItem:
        document_kind = value.get("document_kind")
        return cls(
            key=BomItemKey.from_dict(value["key"]),
            document_path=_optional_str(value.get("document_path")),
            document_kind=(
                None if document_kind is None else DocumentKind(document_kind)
            ),
            display_name=_optional_str(value.get("display_name")),
            quantity=int(value["quantity"]),
            suppressed_quantity=int(value["suppressed_quantity"]),
            excluded_quantity=int(value["excluded_quantity"]),
            unknown_state_quantity=int(value["unknown_state_quantity"]),
            minimum_depth=int(value["minimum_depth"]),
            is_virtual=_optional_bool(value.get("is_virtual")),
            occurrence_ids=tuple(str(item) for item in value.get("occurrence_ids", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "key": self.key.to_dict(),
            "document_path": self.document_path,
            "document_kind": (
                None if self.document_kind is None else self.document_kind.value
            ),
            "display_name": self.display_name,
            "quantity": self.quantity,
            "suppressed_quantity": self.suppressed_quantity,
            "excluded_quantity": self.excluded_quantity,
            "unknown_state_quantity": self.unknown_state_quantity,
            "minimum_depth": self.minimum_depth,
            "is_virtual": self.is_virtual,
            "occurrence_ids": list(self.occurrence_ids),
        }


@dataclass(frozen=True, slots=True)
class BomSummary:
    occurrence_count: int
    item_count: int
    part_item_count: int
    assembly_item_count: int
    unresolved_item_count: int
    children_state_counts: Mapping[str, int]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> BomSummary:
        return cls(
            occurrence_count=int(value["occurrence_count"]),
            item_count=int(value["item_count"]),
            part_item_count=int(value["part_item_count"]),
            assembly_item_count=int(value["assembly_item_count"]),
            unresolved_item_count=int(value["unresolved_item_count"]),
            children_state_counts=_int_mapping(value.get("children_state_counts", {})),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "occurrence_count": self.occurrence_count,
            "item_count": self.item_count,
            "part_item_count": self.part_item_count,
            "assembly_item_count": self.assembly_item_count,
            "unresolved_item_count": self.unresolved_item_count,
            "children_state_counts": dict(self.children_state_counts),
        }


@dataclass(frozen=True, slots=True)
class BillOfMaterials:
    schema_version: int
    status: BomStatus
    root_node_id: str | None
    root_configuration: str | None
    root_configuration_index: int | None
    options: BomOptions
    occurrences: tuple[BomOccurrence, ...]
    items: tuple[BomItem, ...]
    summary: BomSummary
    diagnostics: tuple[Diagnostic, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> BillOfMaterials:
        return cls(
            schema_version=int(value["schema_version"]),
            status=BomStatus(value["status"]),
            root_node_id=_optional_str(value.get("root_node_id")),
            root_configuration=_optional_str(value.get("root_configuration")),
            root_configuration_index=_optional_int(
                value.get("root_configuration_index")
            ),
            options=BomOptions.from_dict(value["options"]),
            occurrences=tuple(
                BomOccurrence.from_dict(item) for item in value.get("occurrences", [])
            ),
            items=tuple(BomItem.from_dict(item) for item in value.get("items", [])),
            summary=BomSummary.from_dict(value["summary"]),
            diagnostics=tuple(
                Diagnostic.from_dict(item) for item in value.get("diagnostics", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema_version": self.schema_version,
            "status": self.status.value,
            "root_node_id": self.root_node_id,
            "root_configuration": self.root_configuration,
            "root_configuration_index": self.root_configuration_index,
            "options": self.options.to_dict(),
            "occurrences": [item.to_dict() for item in self.occurrences],
            "items": [item.to_dict() for item in self.items],
            "summary": self.summary.to_dict(),
            "diagnostics": [item.to_dict() for item in self.diagnostics],
        }

    def part_items(self) -> tuple[BomItem, ...]:
        """Return the aggregated items that resolved to Part documents."""
        return tuple(
            item for item in self.items if item.document_kind is DocumentKind.PART
        )

    def children_of(self, occurrence_id: str) -> tuple[BomOccurrence, ...]:
        """Return the direct child occurrences of ``occurrence_id`` in source order."""
        return tuple(
            item for item in self.occurrences if item.parent_id == occurrence_id
        )


def _optional_sourced(
    value: Any,
    converter: Callable[[Any], T],
) -> SourcedValue[T] | None:
    if value is None:
        return None
    if not isinstance(value, Mapping):
        raise TypeError("sourced value must be an object")
    return SourcedValue.from_dict(value, converter)


def _sourced_dict(value: SourcedValue[Any] | None) -> dict[str, Any] | None:
    return None if value is None else value.to_dict()


def _string_tuple(value: Any) -> tuple[str, ...]:
    return tuple(str(item) for item in value)


def _string_triple(value: Any) -> tuple[str, str, str]:
    items = tuple(str(item) for item in value)
    if len(items) != 3:
        raise ValueError("expected exactly three source values")
    return items[0], items[1], items[2]


def _optional_str(value: Any) -> str | None:
    return None if value is None else str(value)


def _optional_int(value: Any) -> int | None:
    return None if value is None else int(value)


def _optional_bool(value: Any) -> bool | None:
    return None if value is None else bool(value)


def _optional_float(value: Any) -> float | None:
    return None if value is None else float(value)


def _optional_float_pair(value: Any) -> tuple[float, float] | None:
    if value is None:
        return None
    items = tuple(float(item) for item in value)
    if len(items) != 2:
        raise ValueError("expected exactly two floating-point values")
    return items[0], items[1]


def _optional_float_quad(value: Any) -> tuple[float, float, float, float] | None:
    if value is None:
        return None
    items = tuple(float(item) for item in value)
    if len(items) != 4:
        raise ValueError("expected exactly four floating-point values")
    return items[0], items[1], items[2], items[3]


def _optional_optional_float_quad(
    value: Any,
) -> tuple[float | None, float | None, float | None, float | None] | None:
    if value is None:
        return None
    items = tuple(_optional_float(item) for item in value)
    if len(items) != 4:
        raise ValueError("expected exactly four optional floating-point values")
    return items[0], items[1], items[2], items[3]


def _optional_bool_quad(value: Any) -> tuple[bool, bool, bool, bool] | None:
    if value is None:
        return None
    items = tuple(bool(item) for item in value)
    if len(items) != 4:
        raise ValueError("expected exactly four boolean values")
    return items[0], items[1], items[2], items[3]


def _float_triple(value: Any) -> tuple[float, float, float]:
    items = tuple(float(item) for item in value)
    if len(items) != 3:
        raise ValueError("expected exactly three floating-point values")
    return items[0], items[1], items[2]


def _int_pair(value: Any) -> tuple[int, int]:
    items = tuple(int(item) for item in value)
    if len(items) != 2:
        raise ValueError("expected exactly two integer values")
    return items[0], items[1]


def _int_triple(value: Any) -> tuple[int, int, int]:
    items = tuple(int(item) for item in value)
    if len(items) != 3:
        raise ValueError("expected exactly three integer values")
    return items[0], items[1], items[2]


def _int_mapping(value: Mapping[str, Any]) -> dict[str, int]:
    return {str(key): int(item) for key, item in value.items()}
