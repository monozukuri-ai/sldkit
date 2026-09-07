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
    DOCUMENT_RELATIVE = "document_relative"
    DOCUMENT_BASENAME = "document_basename"
    PROJECT_ROOT_RELATIVE = "project_root_relative"
    PROJECT_ROOT_BASENAME = "project_root_basename"
    WINDOWS_PREFIX_MAPPING = "windows_prefix_mapping"
    SEARCH_DIRECTORY_RELATIVE = "search_directory_relative"
    SEARCH_DIRECTORY_BASENAME = "search_directory_basename"
    HOST_ABSOLUTE = "host_absolute"


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
        return result


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

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> Configuration:
        preview = value.get("preview")
        mass_properties = value.get("mass_properties")
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
class DrawingSheet:
    source_id: str | None
    name: SourcedValue[str] | None
    preview: BinaryResource | None
    views: tuple[DrawingView, ...]

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> DrawingSheet:
        preview = value.get("preview")
        return cls(
            source_id=_optional_str(value.get("source_id")),
            name=_optional_sourced(value.get("name"), str),
            preview=(None if preview is None else BinaryResource.from_dict(preview)),
            views=tuple(DrawingView.from_dict(item) for item in value.get("views", [])),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source_id": self.source_id,
            "name": _sourced_dict(self.name),
            "preview": None if self.preview is None else self.preview.to_dict(),
            "views": [item.to_dict() for item in self.views],
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


@dataclass(frozen=True, slots=True)
class GeometryLoop:
    id: str
    face_id: str
    boundary_role: str
    coedge_ids: tuple[str, ...]
    vertex_uses: tuple[GeometryVertexUse, ...]
    provenance: GeometryEntityProvenance

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> GeometryLoop:
        return cls(
            id=str(value["id"]),
            face_id=str(value["face_id"]),
            boundary_role=str(value["boundary_role"]),
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


@dataclass(frozen=True, slots=True)
class GeometryEdge:
    id: str
    curve_id: str | None
    start_vertex_id: str
    end_vertex_id: str
    parameter_range: tuple[float, float] | None
    tolerance: float | None
    provenance: GeometryEntityProvenance

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
        )

    def to_dict(self) -> dict[str, Any]:
        return {
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

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> SourceDocument:
        internal_version = value.get("internal_version")
        preview = value.get("preview")
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
        )

    def to_dict(self) -> dict[str, Any]:
        return {
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

    @classmethod
    def from_dict(cls, value: Mapping[str, Any]) -> ProjectNode:
        parse_status = value.get("parse_status")
        document_kind = value.get("document_kind")
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
        )

    def to_dict(self) -> dict[str, Any]:
        return {
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
        )

    def to_dict(self) -> dict[str, Any]:
        return {
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
