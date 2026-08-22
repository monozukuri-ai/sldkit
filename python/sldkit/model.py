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
                str(key): str(item)
                for key, item in value.get("attributes", {}).items()
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
                str(key): str(item)
                for key, item in value.get("attributes", {}).items()
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
                None
                if inventory is None
                else ContainerInventory.from_dict(inventory)
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
            "inventory": (
                None if self.inventory is None else self.inventory.to_dict()
            ),
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
            document_kind=_optional_sourced(
                value.get("document_kind"), DocumentKind
            ),
            referenced_configuration=_optional_sourced(
                value.get("referenced_configuration"), str
            ),
            component_reference=_optional_sourced(
                value.get("component_reference"), str
            ),
            is_suppressed=_optional_sourced(value.get("is_suppressed"), bool),
            is_hidden=_optional_sourced(value.get("is_hidden"), bool),
            exclude_from_bom=_optional_sourced(
                value.get("exclude_from_bom"), bool
            ),
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
            "referenced_configuration": _sourced_dict(
                self.referenced_configuration
            ),
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
            preview=(
                None if preview is None else BinaryResource.from_dict(preview)
            ),
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
                None
                if self.mass_properties is None
                else self.mass_properties.to_dict()
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
            document_kind=_optional_sourced(
                value.get("document_kind"), DocumentKind
            ),
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
            "referenced_configuration": _sourced_dict(
                self.referenced_configuration
            ),
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
            preview=(
                None if preview is None else BinaryResource.from_dict(preview)
            ),
            views=tuple(
                DrawingView.from_dict(item) for item in value.get("views", [])
            ),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "source_id": self.source_id,
            "name": _sourced_dict(self.name),
            "preview": None if self.preview is None else self.preview.to_dict(),
            "views": [item.to_dict() for item in self.views],
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
            preview=(
                None if preview is None else BinaryResource.from_dict(preview)
            ),
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
                None
                if inventory is None
                else ContainerInventory.from_dict(inventory)
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
            "inventory": (
                None if self.inventory is None else self.inventory.to_dict()
            ),
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
            parse_status=(
                None if parse_status is None else ParseStatus(parse_status)
            ),
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
            "selected_configuration_indices": list(
                self.selected_configuration_indices
            ),
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


def _int_mapping(value: Mapping[str, Any]) -> dict[str, int]:
    return {str(key): int(item) for key, item in value.items()}
