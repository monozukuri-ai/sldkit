use std::collections::{BTreeMap, BTreeSet};

use crc32fast::Hasher as Crc32;
use sha2::{Digest, Sha256};
use sldkit_container::decode_selected_bytes;
use sldkit_core::{
    BinaryResource, BinaryResourceKind, Configuration, ContainerInventory, CustomProperty,
    Diagnostic, DiagnosticKind, DiagnosticSeverity, DocumentKind, InventoryEntry,
    InventoryEntryState, PropertyKind, PropertyScope, PropertyValueState, RecordOffsetBasis,
    ResourceLimits, SemanticCoverage, SourceValue, UnknownRecord, ValueOrigin,
};

const FMTID_SUMMARY_INFORMATION: [u8; 16] = [
    0xe0, 0x85, 0x9f, 0xf2, 0xf9, 0x4f, 0x68, 0x10, 0xab, 0x91, 0x08, 0x00, 0x2b, 0x27, 0xb3, 0xd9,
];
const FMTID_DOCUMENT_SUMMARY_INFORMATION: [u8; 16] = [
    0x02, 0xd5, 0xcd, 0xd5, 0x9c, 0x2e, 0x1b, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2b, 0x2c, 0xf9, 0xae,
];
const FMTID_USER_DEFINED_PROPERTIES: [u8; 16] = [
    0x05, 0xd5, 0xcd, 0xd5, 0x9c, 0x2e, 0x1b, 0x10, 0x93, 0x97, 0x08, 0x00, 0x2b, 0x2c, 0xf9, 0xae,
];
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
const PART_ROOT_CLSID: &str = "83a33d30-27c5-11ce-bfd4-00400513bb57";
const ASSEMBLY_ROOT_CLSID: &str = "83a33d36-27c5-11ce-bfd4-00400513bb57";

pub(crate) struct LegacyFacts {
    pub document_kind: SourceValue<DocumentKind>,
    pub internal_version: Option<SourceValue<u64>>,
    pub configurations: Vec<Configuration>,
    pub properties: Vec<CustomProperty>,
    pub preview: Option<BinaryResource>,
    pub unknown_records: Vec<UnknownRecord>,
    pub diagnostics: Vec<Diagnostic>,
    pub semantic_coverage: SemanticCoverage,
    pub rejected: bool,
}

pub(crate) fn is_solidworks_candidate(inventory: &ContainerInventory) -> bool {
    inventory.entries.iter().any(|entry| {
        let path = entry.path.as_deref().unwrap_or_default();
        let solidworks_root = path == "/"
            && entry.attributes.get("clsid").is_some_and(|clsid| {
                matches!(clsid.as_str(), PART_ROOT_CLSID | ASSEMBLY_ROOT_CLSID)
            });
        let version_storage = path.split('/').any(|component| {
            component.starts_with("_MO_VERSION_") || component.starts_with("_DL_VERSION_")
        });
        let application_stream = matches!(
            path,
            "/ISolidWorksInformation" | "/Contents/CMgr" | "/Contents/CMgrHdr2"
        );
        solidworks_root || version_storage || application_stream
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SemanticClass {
    Uninterpreted,
    FullyInterpreted,
    PartiallyInterpreted,
    Malformed,
}

#[derive(Default)]
struct ConfigCandidate {
    index_evidence: Vec<String>,
    name: Option<SourceValue<String>>,
    parent_index: Option<SourceValue<i64>>,
}

#[derive(Default)]
struct Builder {
    versions: BTreeMap<u64, Vec<String>>,
    configs: BTreeMap<i64, ConfigCandidate>,
    properties: Vec<CustomProperty>,
    preview: Option<BinaryResource>,
    config_previews: BTreeMap<i64, BinaryResource>,
    diagnostics: Vec<Diagnostic>,
    classes: BTreeMap<String, SemanticClass>,
    rejected: bool,
}

impl Builder {
    fn mark(&mut self, entry_id: &str, class: SemanticClass) {
        self.classes
            .entry(entry_id.to_owned())
            .and_modify(|current| *current = (*current).max(class))
            .or_insert(class);
    }

    fn ensure_config(&mut self, index: i64, evidence: Vec<String>) -> &mut ConfigCandidate {
        let candidate = self.configs.entry(index).or_default();
        for item in evidence {
            if !candidate.index_evidence.contains(&item) {
                candidate.index_evidence.push(item);
            }
        }
        candidate
    }
}

pub(crate) fn decode(
    data: &[u8],
    inventory: &ContainerInventory,
    filename_kind: &SourceValue<DocumentKind>,
    limits: &ResourceLimits,
) -> LegacyFacts {
    let wanted = inventory
        .entries
        .iter()
        .filter(|entry| {
            entry.state == InventoryEntryState::Decoded
                && entry.path.as_deref().is_some_and(is_selected_stream)
        })
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    let decoded = decode_selected_bytes(data, &wanted, limits);
    let mut builder = Builder::default();

    collect_version_candidates(inventory, &mut builder);
    collect_configuration_path_candidates(inventory, &mut builder);
    let internal_version = resolve_version(&mut builder);

    for entry in &inventory.entries {
        let Some(path) = entry.path.as_deref() else {
            continue;
        };
        if entry.state != InventoryEntryState::Decoded || !is_selected_stream(path) {
            continue;
        }
        let Some(bytes) = decoded.streams.get(&entry.id) else {
            builder.mark(&entry.id, SemanticClass::Malformed);
            builder.diagnostics.push(
                Diagnostic::new(
                    "legacy.selected_stream_unavailable",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "a decoded legacy stream was unavailable during semantic parsing",
                )
                .in_stream(path)
                .with_detail("entry_id", &entry.id),
            );
            continue;
        };
        decode_stream(
            entry,
            bytes,
            internal_version.as_ref().map(|value| value.value),
            limits,
            &mut builder,
        );
    }

    let document_kind = resolve_document_kind(inventory, filename_kind, &mut builder);
    let configurations = finish_configurations(&mut builder);
    let (semantic_coverage, unknown_records) = semantic_accounting(inventory, &builder.classes);

    LegacyFacts {
        document_kind,
        internal_version,
        configurations,
        properties: builder.properties,
        preview: builder.preview,
        unknown_records,
        diagnostics: builder.diagnostics,
        semantic_coverage,
        rejected: builder.rejected,
    }
}

fn is_selected_stream(path: &str) -> bool {
    matches!(
        path,
        "/\u{5}SummaryInformation"
            | "/\u{5}DocumentSummaryInformation"
            | "/ISolidWorksInformation"
            | "/Contents/CMgrHdr2"
            | "/Preview"
            | "/PreviewPNG"
    ) || config_index_from_suffix(path, "-Properties").is_some()
        || config_index_from_suffix(path, "-Preview").is_some()
        || config_index_from_suffix(path, "-PreviewPNG").is_some()
}

fn decode_stream(
    entry: &InventoryEntry,
    bytes: &[u8],
    internal_version: Option<u64>,
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    let path = entry.path.as_deref().unwrap_or_default();
    match path {
        "/\u{5}SummaryInformation" | "/\u{5}DocumentSummaryInformation" => {
            decode_property_stream(
                entry,
                bytes,
                PropertyKind::Core,
                PropertyScope::Global,
                None,
                limits,
                builder,
            );
        }
        "/ISolidWorksInformation" => decode_property_stream(
            entry,
            bytes,
            PropertyKind::System,
            PropertyScope::Global,
            None,
            limits,
            builder,
        ),
        "/Contents/CMgrHdr2" => {
            decode_configuration_header(entry, bytes, internal_version, limits, builder);
        }
        "/Preview" | "/PreviewPNG" => decode_document_preview(entry, bytes, limits, builder),
        _ => {
            if let Some(index) = config_index_from_suffix(path, "-Properties") {
                builder.ensure_config(index, evidence(entry, "stream_path"));
                decode_property_stream(
                    entry,
                    bytes,
                    PropertyKind::Custom,
                    PropertyScope::Configuration,
                    Some(index),
                    limits,
                    builder,
                );
            } else if let Some(index) = config_index_from_suffix(path, "-PreviewPNG")
                .or_else(|| config_index_from_suffix(path, "-Preview"))
            {
                builder.ensure_config(index, evidence(entry, "stream_path"));
                decode_configuration_preview(entry, bytes, index, limits, builder);
            }
        }
    }
}

fn collect_version_candidates(inventory: &ContainerInventory, builder: &mut Builder) {
    for entry in &inventory.entries {
        let Some(path) = entry.path.as_deref() else {
            continue;
        };
        for component in path.split('/') {
            let Some(raw) = component.strip_prefix("_MO_VERSION_") else {
                continue;
            };
            let Ok(version) = raw.parse::<u64>() else {
                continue;
            };
            builder
                .versions
                .entry(version)
                .or_default()
                .push(format!("{}:stream_path", entry.id));
        }
    }
}

fn resolve_version(builder: &mut Builder) -> Option<SourceValue<u64>> {
    if builder.versions.len() == 1 {
        let (&version, evidence) = builder.versions.iter().next()?;
        return Some(SourceValue::new(
            version,
            ValueOrigin::Source,
            evidence.clone(),
        ));
    }
    if builder.versions.len() > 1 {
        builder.diagnostics.push(
            Diagnostic::new(
                "legacy.internal_version_ambiguous",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Preserved,
                "multiple legacy model-version storage names were preserved",
            )
            .with_detail(
                "versions",
                builder
                    .versions
                    .keys()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        );
    }
    None
}

fn collect_configuration_path_candidates(inventory: &ContainerInventory, builder: &mut Builder) {
    for entry in &inventory.entries {
        let Some(path) = entry.path.as_deref() else {
            continue;
        };
        if let Some(index) = config_index_from_contents_path(path)
            .or_else(|| config_index_from_suffix(path, "-Properties"))
            .or_else(|| config_index_from_suffix(path, "-Preview"))
            .or_else(|| config_index_from_suffix(path, "-PreviewPNG"))
        {
            builder.ensure_config(index, evidence(entry, "stream_path"));
        }
    }
}

fn config_index_from_contents_path(path: &str) -> Option<i64> {
    let rest = path.strip_prefix("/Contents/Config-")?;
    let digits = rest
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    (!digits.is_empty()).then(|| digits.parse::<i64>().ok())?
}

fn config_index_from_suffix(path: &str, suffix: &str) -> Option<i64> {
    let raw = path.strip_prefix("/Config-")?.strip_suffix(suffix)?;
    raw.parse().ok()
}

fn resolve_document_kind(
    inventory: &ContainerInventory,
    filename_kind: &SourceValue<DocumentKind>,
    builder: &mut Builder,
) -> SourceValue<DocumentKind> {
    let source_kind = inventory.entries.iter().find_map(|entry| {
        if entry.path.as_deref() != Some("/") {
            return None;
        }
        let clsid = entry.attributes.get("clsid")?;
        let kind = match clsid.as_str() {
            PART_ROOT_CLSID => DocumentKind::Part,
            ASSEMBLY_ROOT_CLSID => DocumentKind::Assembly,
            _ => return None,
        };
        Some(SourceValue::new(
            kind,
            ValueOrigin::Source,
            evidence(entry, "root_storage_clsid"),
        ))
    });

    if let Some(source_kind) = source_kind {
        if filename_kind.value != DocumentKind::Unknown && filename_kind.value != source_kind.value
        {
            builder.diagnostics.push(
                Diagnostic::new(
                    "input.document_kind_mismatch",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "content-derived document kind takes precedence over the filename extension",
                )
                .with_detail("content_kind", document_kind_name(source_kind.value))
                .with_detail("filename_kind", document_kind_name(filename_kind.value)),
            );
        }
        source_kind
    } else {
        filename_kind.clone()
    }
}

fn finish_configurations(builder: &mut Builder) -> Vec<Configuration> {
    let previews = std::mem::take(&mut builder.config_previews);
    builder
        .configs
        .iter()
        .map(|(&index, candidate)| Configuration {
            index: SourceValue::new(index, ValueOrigin::Source, candidate.index_evidence.clone()),
            name: candidate.name.clone(),
            alternate_names: Vec::new(),
            parent_name: None,
            parent_index: candidate.parent_index.clone(),
            preview: previews.get(&index).cloned(),
            mass_properties: None,
            components: Vec::new(),
        })
        .collect()
}

fn semantic_accounting(
    inventory: &ContainerInventory,
    classes: &BTreeMap<String, SemanticClass>,
) -> (SemanticCoverage, Vec<UnknownRecord>) {
    let mut coverage = SemanticCoverage::default();
    let mut unknown = Vec::new();
    for entry in &inventory.entries {
        if entry.state != InventoryEntryState::Decoded {
            continue;
        }
        let size = entry.decoded_size.unwrap_or(0);
        coverage.decoded_streams_total = coverage.decoded_streams_total.saturating_add(1);
        coverage.decoded_bytes_total = coverage.decoded_bytes_total.saturating_add(size);
        let class = classes
            .get(&entry.id)
            .copied()
            .unwrap_or(SemanticClass::Uninterpreted);
        let suffix = match class {
            SemanticClass::FullyInterpreted => {
                coverage.fully_interpreted_streams =
                    coverage.fully_interpreted_streams.saturating_add(1);
                coverage.fully_interpreted_bytes =
                    coverage.fully_interpreted_bytes.saturating_add(size);
                None
            }
            SemanticClass::PartiallyInterpreted => {
                coverage.partially_interpreted_streams =
                    coverage.partially_interpreted_streams.saturating_add(1);
                coverage.partially_interpreted_bytes =
                    coverage.partially_interpreted_bytes.saturating_add(size);
                Some("partially_interpreted")
            }
            SemanticClass::Uninterpreted => {
                coverage.uninterpreted_streams = coverage.uninterpreted_streams.saturating_add(1);
                coverage.uninterpreted_bytes = coverage.uninterpreted_bytes.saturating_add(size);
                Some("unsupported")
            }
            SemanticClass::Malformed => {
                coverage.malformed_streams = coverage.malformed_streams.saturating_add(1);
                coverage.malformed_bytes = coverage.malformed_bytes.saturating_add(size);
                Some("malformed")
            }
        };
        if let Some(suffix) = suffix {
            let family = entry
                .attributes
                .get("legacy_stream_family")
                .map_or("other", String::as_str);
            unknown.push(UnknownRecord {
                entry_id: Some(entry.id.clone()),
                stream_path: entry.path.clone(),
                record_kind: None,
                offset_basis: RecordOffsetBasis::DecodedStream,
                offset: 0,
                length: size,
                sha256: entry.decoded_sha256.clone().unwrap_or_default(),
                reason_code: format!("legacy.{family}.{suffix}"),
            });
        }
    }
    (coverage, unknown)
}

fn evidence(entry: &InventoryEntry, field: &str) -> Vec<String> {
    vec![format!("{}:{field}", entry.id)]
}

const fn document_kind_name(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::Part => "part",
        DocumentKind::Assembly => "assembly",
        DocumentKind::Drawing => "drawing",
        DocumentKind::Unknown => "unknown",
    }
}

#[derive(Clone, Copy)]
struct SectionDescriptor {
    format_id: [u8; 16],
    offset: usize,
}

#[derive(Clone, Copy)]
struct PropertyDescriptor {
    identifier: u32,
    offset: usize,
    end: usize,
}

struct PropertyDecode {
    raw_value: Option<String>,
    value_type: String,
    state: PropertyValueState,
    fully_interpreted: bool,
    diagnostics: Vec<Diagnostic>,
}

struct PropertyStreamDecode {
    properties: Vec<CustomProperty>,
    diagnostics: Vec<Diagnostic>,
    fully_interpreted: bool,
}

#[derive(Clone, Copy)]
struct DecodeFailure {
    code: &'static str,
    message: &'static str,
    offset: usize,
    severity: DiagnosticSeverity,
    kind: DiagnosticKind,
}

impl DecodeFailure {
    const fn malformed(code: &'static str, message: &'static str, offset: usize) -> Self {
        Self {
            code,
            message,
            offset,
            severity: DiagnosticSeverity::Error,
            kind: DiagnosticKind::Malformed,
        }
    }

    const fn unsupported(code: &'static str, message: &'static str, offset: usize) -> Self {
        Self {
            code,
            message,
            offset,
            severity: DiagnosticSeverity::Warning,
            kind: DiagnosticKind::Unsupported,
        }
    }

    const fn limit(code: &'static str, message: &'static str, offset: usize) -> Self {
        Self {
            code,
            message,
            offset,
            severity: DiagnosticSeverity::Error,
            kind: DiagnosticKind::Fatal,
        }
    }
}

fn decode_property_stream(
    entry: &InventoryEntry,
    bytes: &[u8],
    default_kind: PropertyKind,
    scope: PropertyScope,
    configuration_index: Option<i64>,
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    match parse_property_stream(
        entry,
        bytes,
        default_kind,
        scope,
        configuration_index,
        limits,
    ) {
        Ok(decoded) => {
            builder.properties.extend(decoded.properties);
            builder.diagnostics.extend(decoded.diagnostics);
            builder.mark(
                &entry.id,
                if decoded.fully_interpreted {
                    SemanticClass::FullyInterpreted
                } else {
                    SemanticClass::PartiallyInterpreted
                },
            );
        }
        Err(failure) => {
            if failure.kind == DiagnosticKind::Fatal {
                builder.rejected = true;
            }
            builder.mark(
                &entry.id,
                if failure.kind == DiagnosticKind::Malformed {
                    SemanticClass::Malformed
                } else {
                    SemanticClass::Uninterpreted
                },
            );
            builder.diagnostics.push(failure_diagnostic(entry, failure));
        }
    }
}

#[allow(clippy::too_many_lines)]
fn parse_property_stream(
    entry: &InventoryEntry,
    bytes: &[u8],
    default_kind: PropertyKind,
    scope: PropertyScope,
    configuration_index: Option<i64>,
    limits: &ResourceLimits,
) -> Result<PropertyStreamDecode, DecodeFailure> {
    if u16_at(bytes, 0) != Some(0xfffe) {
        return Err(DecodeFailure::malformed(
            "legacy.property_set_byte_order_invalid",
            "OLE property set does not declare little-endian byte order",
            0,
        ));
    }
    let version = u16_at(bytes, 2).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.property_set_header_truncated",
            "OLE property set ends inside its stream header",
            2,
        )
    })?;
    if version > 1 {
        return Err(DecodeFailure::unsupported(
            "legacy.property_set_version_unsupported",
            "OLE property set version is outside the supported profile",
            2,
        ));
    }
    let section_count = u32_at(bytes, 24).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.property_set_header_truncated",
            "OLE property set ends before its section count",
            24,
        )
    })?;
    if u64::from(section_count) > limits.max_stream_count {
        return Err(DecodeFailure::limit(
            "limit.property_set_sections",
            "OLE property set section count exceeds the configured stream limit",
            24,
        ));
    }
    let section_capacity = usize::try_from(section_count).map_err(|_| {
        DecodeFailure::limit(
            "limit.property_set_sections",
            "OLE property set section count exceeds the host numeric range",
            24,
        )
    })?;
    let descriptors_bytes = section_capacity.checked_mul(20).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property set descriptor size overflows the host numeric range",
            24,
        )
    })?;
    let header_end = 28_usize.checked_add(descriptors_bytes).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property set header size overflows the host numeric range",
            24,
        )
    })?;
    if bytes.len() < header_end {
        return Err(DecodeFailure::malformed(
            "legacy.property_set_header_truncated",
            "OLE property set ends inside its section descriptors",
            bytes.len(),
        ));
    }

    let mut sections = Vec::with_capacity(section_capacity);
    for index in 0..section_capacity {
        let descriptor_offset = 28 + index * 20;
        let format_id: [u8; 16] = bytes[descriptor_offset..descriptor_offset + 16]
            .try_into()
            .map_err(|_| {
                DecodeFailure::malformed(
                    "legacy.property_set_header_truncated",
                    "OLE property set ends inside a format identifier",
                    descriptor_offset,
                )
            })?;
        let section_offset =
            usize::try_from(u32_at(bytes, descriptor_offset + 16).ok_or_else(|| {
                DecodeFailure::malformed(
                    "legacy.property_set_header_truncated",
                    "OLE property set ends inside a section offset",
                    descriptor_offset + 16,
                )
            })?)
            .map_err(|_| {
                DecodeFailure::limit(
                    "limit.numeric_range",
                    "OLE property set section offset exceeds the host numeric range",
                    descriptor_offset + 16,
                )
            })?;
        if section_offset < header_end || section_offset >= bytes.len() {
            return Err(DecodeFailure::malformed(
                "legacy.property_set_section_offset_invalid",
                "OLE property set section offset is outside the stream",
                descriptor_offset + 16,
            ));
        }
        sections.push(SectionDescriptor {
            format_id,
            offset: section_offset,
        });
    }

    let mut properties = Vec::new();
    let mut diagnostics = Vec::new();
    let mut fully_interpreted = true;
    for section in sections {
        let decoded = parse_property_section(
            entry,
            bytes,
            section,
            default_kind,
            scope,
            configuration_index,
            limits,
        )?;
        properties.extend(decoded.properties);
        diagnostics.extend(decoded.diagnostics);
        fully_interpreted &= decoded.fully_interpreted;
    }
    let mut missing_code_page_properties = 0_usize;
    diagnostics.retain(|diagnostic| {
        if diagnostic.code == "legacy.property_code_page_missing" {
            missing_code_page_properties = missing_code_page_properties.saturating_add(1);
            false
        } else {
            true
        }
    });
    if missing_code_page_properties > 0 {
        diagnostics.push(
            Diagnostic::new(
                "legacy.property_code_page_missing",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "ASCII-only properties were decoded without a declared code page",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail(
                "affected_properties",
                missing_code_page_properties.to_string(),
            ),
        );
    }

    Ok(PropertyStreamDecode {
        properties,
        diagnostics,
        fully_interpreted,
    })
}

#[allow(clippy::too_many_lines)]
fn parse_property_section(
    entry: &InventoryEntry,
    bytes: &[u8],
    section: SectionDescriptor,
    default_kind: PropertyKind,
    scope: PropertyScope,
    configuration_index: Option<i64>,
    limits: &ResourceLimits,
) -> Result<PropertyStreamDecode, DecodeFailure> {
    let section_size = usize::try_from(u32_at(bytes, section.offset).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.property_set_section_truncated",
            "OLE property section ends before its size",
            section.offset,
        )
    })?)
    .map_err(|_| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property section size exceeds the host numeric range",
            section.offset,
        )
    })?;
    let section_end = section.offset.checked_add(section_size).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property section range overflows the host numeric range",
            section.offset,
        )
    })?;
    if section_size < 8 || section_end > bytes.len() {
        return Err(DecodeFailure::malformed(
            "legacy.property_set_section_truncated",
            "OLE property section extends beyond the decoded stream",
            section.offset,
        ));
    }
    let property_count = u32_at(bytes, section.offset + 4).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.property_set_section_truncated",
            "OLE property section ends before its property count",
            section.offset + 4,
        )
    })?;
    if u64::from(property_count) > limits.max_stream_count {
        return Err(DecodeFailure::limit(
            "limit.property_count",
            "OLE property count exceeds the configured stream limit",
            section.offset + 4,
        ));
    }
    let property_capacity = usize::try_from(property_count).map_err(|_| {
        DecodeFailure::limit(
            "limit.property_count",
            "OLE property count exceeds the host numeric range",
            section.offset + 4,
        )
    })?;
    let table_bytes = property_capacity.checked_mul(8).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property table size overflows the host numeric range",
            section.offset + 4,
        )
    })?;
    let table_end_relative = 8_usize.checked_add(table_bytes).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property table range overflows the host numeric range",
            section.offset + 4,
        )
    })?;
    if table_end_relative > section_size {
        return Err(DecodeFailure::malformed(
            "legacy.property_set_table_truncated",
            "OLE property table extends beyond its section",
            section.offset + 8,
        ));
    }

    let mut raw_descriptors = Vec::with_capacity(property_capacity);
    for index in 0..property_capacity {
        let pair_offset = section.offset + 8 + index * 8;
        let identifier = u32_at(bytes, pair_offset).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.property_set_table_truncated",
                "OLE property table ends inside an identifier",
                pair_offset,
            )
        })?;
        let relative = usize::try_from(u32_at(bytes, pair_offset + 4).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.property_set_table_truncated",
                "OLE property table ends inside an offset",
                pair_offset + 4,
            )
        })?)
        .map_err(|_| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "OLE property offset exceeds the host numeric range",
                pair_offset + 4,
            )
        })?;
        if relative < table_end_relative || relative >= section_size {
            return Err(DecodeFailure::malformed(
                "legacy.property_set_property_offset_invalid",
                "OLE property offset is outside the property payload region",
                pair_offset + 4,
            ));
        }
        raw_descriptors.push((identifier, relative));
    }
    raw_descriptors.sort_by_key(|(_, offset)| *offset);
    if raw_descriptors
        .windows(2)
        .any(|pair| pair[0].1 == pair[1].1)
    {
        return Err(DecodeFailure::malformed(
            "legacy.property_set_duplicate_offset",
            "multiple OLE properties share one payload offset",
            section.offset + 8,
        ));
    }
    let descriptors = raw_descriptors
        .iter()
        .enumerate()
        .map(|(index, &(identifier, relative))| PropertyDescriptor {
            identifier,
            offset: section.offset + relative,
            end: raw_descriptors
                .get(index + 1)
                .map_or(section_end, |(_, next)| section.offset + *next),
        })
        .collect::<Vec<_>>();

    let code_page = descriptors
        .iter()
        .find(|property| property.identifier == 1)
        .and_then(|property| property_code_page(bytes, *property));
    let dictionary =
        if let Some(property) = descriptors.iter().find(|property| property.identifier == 0) {
            parse_dictionary(bytes, *property, code_page, limits)?
        } else {
            BTreeMap::new()
        };

    let mut properties = Vec::new();
    let mut diagnostics = Vec::new();
    let mut fully_interpreted = true;
    let format = property_set_format(&section.format_id);
    if format == PropertySetFormat::Unknown {
        fully_interpreted = false;
        diagnostics.push(
            Diagnostic::new(
                "legacy.property_set_format_unknown",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "an application-defined property-set format was decoded generically",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail("format_id", guid_packet_hex(&section.format_id)),
        );
    }

    for property in descriptors {
        if property.identifier == 0 {
            continue;
        }
        let decoded = decode_typed_property(entry, bytes, property, code_page, limits)?;
        fully_interpreted &= decoded.fully_interpreted;
        diagnostics.extend(decoded.diagnostics);
        let (name, name_origin, name_evidence) = property_name(
            entry,
            format,
            property.identifier,
            dictionary.get(&property.identifier),
        );
        let kind = match format {
            PropertySetFormat::Summary | PropertySetFormat::DocumentSummary => PropertyKind::Core,
            PropertySetFormat::UserDefined => {
                if default_kind == PropertyKind::System {
                    PropertyKind::System
                } else {
                    PropertyKind::Custom
                }
            }
            PropertySetFormat::Unknown => default_kind,
        };
        let raw_value = decoded.raw_value.map(|value| {
            SourceValue::new(
                value,
                ValueOrigin::Source,
                evidence(
                    entry,
                    &format!("typed_property@decoded:{}", property.offset),
                ),
            )
        });
        properties.push(CustomProperty {
            name: SourceValue::new(name, name_origin, name_evidence),
            raw_value,
            value_type: Some(decoded.value_type),
            value_state: decoded.state,
            kind,
            scope,
            configuration: None,
            configuration_index,
            stream_path: entry.path.clone().unwrap_or_default(),
            pid: Some(property.identifier),
        });
    }

    Ok(PropertyStreamDecode {
        properties,
        diagnostics,
        fully_interpreted,
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PropertySetFormat {
    Summary,
    DocumentSummary,
    UserDefined,
    Unknown,
}

fn property_set_format(format_id: &[u8; 16]) -> PropertySetFormat {
    if format_id == &FMTID_SUMMARY_INFORMATION {
        PropertySetFormat::Summary
    } else if format_id == &FMTID_DOCUMENT_SUMMARY_INFORMATION {
        PropertySetFormat::DocumentSummary
    } else if format_id == &FMTID_USER_DEFINED_PROPERTIES {
        PropertySetFormat::UserDefined
    } else {
        PropertySetFormat::Unknown
    }
}

fn property_code_page(bytes: &[u8], property: PropertyDescriptor) -> Option<u16> {
    (u16_at(bytes, property.offset) == Some(0x0002)).then(|| u16_at(bytes, property.offset + 4))?
}

#[allow(clippy::too_many_lines)]
fn parse_dictionary(
    bytes: &[u8],
    property: PropertyDescriptor,
    code_page: Option<u16>,
    limits: &ResourceLimits,
) -> Result<BTreeMap<u32, String>, DecodeFailure> {
    let count = u32_at(bytes, property.offset).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.property_dictionary_truncated",
            "OLE property dictionary ends before its entry count",
            property.offset,
        )
    })?;
    if u64::from(count) > limits.max_stream_count {
        return Err(DecodeFailure::limit(
            "limit.property_dictionary_entries",
            "OLE property dictionary entry count exceeds the configured stream limit",
            property.offset,
        ));
    }
    let mut cursor = property.offset + 4;
    let mut dictionary = BTreeMap::new();
    for _ in 0..count {
        let identifier = u32_at(bytes, cursor).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.property_dictionary_truncated",
                "OLE property dictionary ends inside an entry identifier",
                cursor,
            )
        })?;
        let length_offset = cursor + 4;
        let length = usize::try_from(u32_at(bytes, length_offset).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.property_dictionary_truncated",
                "OLE property dictionary ends inside a name length",
                length_offset,
            )
        })?)
        .map_err(|_| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "OLE property dictionary name length exceeds the host numeric range",
                length_offset,
            )
        })?;
        cursor = cursor.checked_add(8).ok_or_else(|| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "OLE property dictionary offset overflows the host numeric range",
                cursor,
            )
        })?;
        let byte_len = if code_page == Some(1200) {
            length.checked_mul(2)
        } else {
            Some(length)
        }
        .ok_or_else(|| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "OLE property dictionary byte length overflows the host numeric range",
                length_offset,
            )
        })?;
        if saturating_u64(byte_len) > limits.max_string_bytes {
            return Err(DecodeFailure::limit(
                "limit.string_bytes",
                "OLE property dictionary name exceeds the configured string limit",
                length_offset,
            ));
        }
        let end = cursor.checked_add(byte_len).ok_or_else(|| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "OLE property dictionary name range overflows the host numeric range",
                cursor,
            )
        })?;
        if end > property.end {
            return Err(DecodeFailure::malformed(
                "legacy.property_dictionary_truncated",
                "OLE property dictionary ends inside a name",
                cursor,
            ));
        }
        let raw = &bytes[cursor..end];
        if let Some(name) = decode_dictionary_name(raw, code_page) {
            dictionary.insert(identifier, name);
        }
        cursor = if code_page == Some(1200) {
            align_four(end).ok_or_else(|| {
                DecodeFailure::limit(
                    "limit.numeric_range",
                    "OLE property dictionary alignment overflows the host numeric range",
                    end,
                )
            })?
        } else {
            end
        };
        if cursor > property.end {
            return Err(DecodeFailure::malformed(
                "legacy.property_dictionary_truncated",
                "OLE property dictionary padding extends beyond its property",
                end,
            ));
        }
    }
    Ok(dictionary)
}

fn decode_dictionary_name(raw: &[u8], code_page: Option<u16>) -> Option<String> {
    if code_page == Some(1200) {
        if !raw.len().is_multiple_of(2) {
            return None;
        }
        let units = raw
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect::<Vec<_>>();
        String::from_utf16(&units).ok()
    } else {
        let value = raw.split(|byte| *byte == 0).next().unwrap_or_default();
        if code_page == Some(65001) || value.is_ascii() {
            std::str::from_utf8(value).ok().map(str::to_owned)
        } else {
            None
        }
    }
}

#[allow(clippy::too_many_lines)]
fn decode_typed_property(
    entry: &InventoryEntry,
    bytes: &[u8],
    property: PropertyDescriptor,
    code_page: Option<u16>,
    limits: &ResourceLimits,
) -> Result<PropertyDecode, DecodeFailure> {
    let type_code = u16_at(bytes, property.offset).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.typed_property_truncated",
            "OLE typed property ends before its type",
            property.offset,
        )
    })?;
    let padding = u16_at(bytes, property.offset + 2).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.typed_property_truncated",
            "OLE typed property ends inside its header",
            property.offset + 2,
        )
    })?;
    let value_offset = property.offset.checked_add(4).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE typed property value offset overflows the host numeric range",
            property.offset,
        )
    })?;
    if value_offset > property.end || property.end > bytes.len() {
        return Err(DecodeFailure::malformed(
            "legacy.typed_property_truncated",
            "OLE typed property value is outside its section",
            property.offset,
        ));
    }
    let mut diagnostics = Vec::new();
    let mut fully_interpreted = true;
    if padding != 0 {
        fully_interpreted = false;
        diagnostics.push(
            Diagnostic::new(
                "legacy.typed_property_padding_nonzero",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Preserved,
                "nonzero OLE typed-property header padding was preserved",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail("decoded_offset", (property.offset + 2).to_string())
            .with_detail("padding", padding.to_string()),
        );
    }

    let value_type = property_type_name(type_code);
    let decoded = match type_code {
        0x0000 | 0x0001 => (None, PropertyValueState::Missing, true),
        0x0002 => {
            let raw = i16_at(bytes, value_offset)
                .ok_or_else(|| typed_value_truncated(property, "16-bit integer"))?;
            let value = if property.identifier == 1 {
                u16_at(bytes, value_offset).map(|value| value.to_string())
            } else {
                Some(raw.to_string())
            };
            (value, PropertyValueState::Present, true)
        }
        0x0003 | 0x0016 => {
            let value = i32_at(bytes, value_offset)
                .ok_or_else(|| typed_value_truncated(property, "32-bit integer"))?;
            (Some(value.to_string()), PropertyValueState::Present, true)
        }
        0x000b => {
            let value = i16_at(bytes, value_offset)
                .ok_or_else(|| typed_value_truncated(property, "boolean"))?;
            match value {
                0 => (Some("false".to_owned()), PropertyValueState::Present, true),
                -1 => (Some("true".to_owned()), PropertyValueState::Present, true),
                _ => (
                    Some(value.to_string()),
                    PropertyValueState::UnsupportedType,
                    false,
                ),
            }
        }
        0x0010 => {
            let value = bytes
                .get(value_offset)
                .copied()
                .ok_or_else(|| typed_value_truncated(property, "8-bit signed integer"))?;
            (
                Some(i8::from_le_bytes([value]).to_string()),
                PropertyValueState::Present,
                true,
            )
        }
        0x0011 => {
            let value = bytes
                .get(value_offset)
                .copied()
                .ok_or_else(|| typed_value_truncated(property, "8-bit unsigned integer"))?;
            (Some(value.to_string()), PropertyValueState::Present, true)
        }
        0x0012 => {
            let value = u16_at(bytes, value_offset)
                .ok_or_else(|| typed_value_truncated(property, "16-bit unsigned integer"))?;
            (Some(value.to_string()), PropertyValueState::Present, true)
        }
        0x0013 | 0x0017 => {
            let value = u32_at(bytes, value_offset)
                .ok_or_else(|| typed_value_truncated(property, "32-bit unsigned integer"))?;
            (Some(value.to_string()), PropertyValueState::Present, true)
        }
        0x0014 => {
            let value = i64_at(bytes, value_offset)
                .ok_or_else(|| typed_value_truncated(property, "64-bit integer"))?;
            (Some(value.to_string()), PropertyValueState::Present, true)
        }
        0x0015 | 0x0040 => {
            let value = u64_at(bytes, value_offset)
                .ok_or_else(|| typed_value_truncated(property, "64-bit unsigned integer"))?;
            (Some(value.to_string()), PropertyValueState::Present, true)
        }
        0x0008 | 0x001e => {
            let value =
                decode_counted_string(bytes, value_offset, property.end, code_page, false, limits)?;
            if value.code_page_missing {
                diagnostics.push(
                    Diagnostic::new(
                        "legacy.property_code_page_missing",
                        DiagnosticSeverity::Info,
                        DiagnosticKind::Preserved,
                        "an ASCII-only property was decoded without a declared code page",
                    )
                    .in_stream(entry.path.as_deref().unwrap_or_default())
                    .with_detail("property_id", property.identifier.to_string()),
                );
            }
            if !value.null_terminated {
                diagnostics.push(
                    Diagnostic::new(
                        "legacy.property_string_not_terminated",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Preserved,
                        "an OLE property string did not contain a terminating null",
                    )
                    .in_stream(entry.path.as_deref().unwrap_or_default())
                    .with_detail("property_id", property.identifier.to_string()),
                );
            }
            (
                value.value,
                value.state,
                value.fully_interpreted && value.null_terminated,
            )
        }
        0x001f => {
            let value =
                decode_counted_string(bytes, value_offset, property.end, Some(1200), true, limits)?;
            if !value.null_terminated {
                diagnostics.push(
                    Diagnostic::new(
                        "legacy.property_string_not_terminated",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Preserved,
                        "an OLE Unicode property string did not contain a terminating null",
                    )
                    .in_stream(entry.path.as_deref().unwrap_or_default())
                    .with_detail("property_id", property.identifier.to_string()),
                );
            }
            (
                value.value,
                value.state,
                value.fully_interpreted && value.null_terminated,
            )
        }
        _ => {
            let raw = bytes.get(value_offset..property.end).and_then(|value| {
                (saturating_u64(value.len()).saturating_mul(2) <= limits.max_string_bytes)
                    .then(|| hex_bytes(value))
            });
            (raw, PropertyValueState::UnsupportedType, false)
        }
    };

    fully_interpreted &= decoded.2;
    if decoded.1 == PropertyValueState::UnsupportedType {
        diagnostics.push(
            Diagnostic::new(
                "legacy.property_type_or_encoding_unsupported",
                DiagnosticSeverity::Info,
                DiagnosticKind::Unsupported,
                "an OLE property value was preserved without semantic normalization",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail("property_id", property.identifier.to_string())
            .with_detail("property_type", value_type.clone()),
        );
    }
    Ok(PropertyDecode {
        raw_value: decoded.0,
        value_type,
        state: decoded.1,
        fully_interpreted,
        diagnostics,
    })
}

struct CountedString {
    value: Option<String>,
    state: PropertyValueState,
    fully_interpreted: bool,
    null_terminated: bool,
    code_page_missing: bool,
}

#[allow(clippy::too_many_lines)]
fn decode_counted_string(
    bytes: &[u8],
    offset: usize,
    property_end: usize,
    code_page: Option<u16>,
    force_utf16: bool,
    limits: &ResourceLimits,
) -> Result<CountedString, DecodeFailure> {
    let count = usize::try_from(u32_at(bytes, offset).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.property_string_truncated",
            "OLE property string ends before its length",
            offset,
        )
    })?)
    .map_err(|_| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property string length exceeds the host numeric range",
            offset,
        )
    })?;
    let utf16 = force_utf16 || code_page == Some(1200);
    let byte_len = if utf16 {
        count.checked_mul(2)
    } else {
        Some(count)
    }
    .ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property string byte length overflows the host numeric range",
            offset,
        )
    })?;
    if saturating_u64(byte_len) > limits.max_string_bytes {
        return Err(DecodeFailure::limit(
            "limit.string_bytes",
            "OLE property string exceeds the configured string limit",
            offset,
        ));
    }
    let value_offset = offset.checked_add(4).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property string offset overflows the host numeric range",
            offset,
        )
    })?;
    let end = value_offset.checked_add(byte_len).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "OLE property string range overflows the host numeric range",
            value_offset,
        )
    })?;
    if end > property_end || end > bytes.len() {
        return Err(DecodeFailure::malformed(
            "legacy.property_string_truncated",
            "OLE property string extends beyond its property payload",
            value_offset,
        ));
    }
    let raw = &bytes[value_offset..end];
    if utf16 {
        if !raw.len().is_multiple_of(2) {
            return Err(DecodeFailure::malformed(
                "legacy.property_string_utf16_invalid",
                "OLE Unicode property string has an odd byte length",
                value_offset,
            ));
        }
        let all_units = raw
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        let terminator = all_units.iter().position(|unit| *unit == 0);
        let units = terminator.map_or(all_units.as_slice(), |index| &all_units[..index]);
        let value = String::from_utf16(units).map_err(|_| {
            DecodeFailure::malformed(
                "legacy.property_string_utf16_invalid",
                "OLE Unicode property string is not valid UTF-16LE",
                value_offset,
            )
        })?;
        let state = if value.is_empty() {
            PropertyValueState::Empty
        } else {
            PropertyValueState::Present
        };
        return Ok(CountedString {
            value: Some(value),
            state,
            fully_interpreted: true,
            null_terminated: terminator.is_some(),
            code_page_missing: false,
        });
    }

    let terminator = raw.iter().position(|byte| *byte == 0);
    let value_bytes = terminator.map_or(raw, |index| &raw[..index]);
    let (value, fully_interpreted, code_page_missing) = match code_page {
        Some(65001) => (
            std::str::from_utf8(value_bytes).ok().map(str::to_owned),
            std::str::from_utf8(value_bytes).is_ok(),
            false,
        ),
        Some(_) => {
            if value_bytes.is_ascii() {
                (
                    std::str::from_utf8(value_bytes).ok().map(str::to_owned),
                    true,
                    false,
                )
            } else {
                (None, false, false)
            }
        }
        None => {
            if value_bytes.is_ascii() {
                (
                    std::str::from_utf8(value_bytes).ok().map(str::to_owned),
                    false,
                    true,
                )
            } else {
                (None, false, true)
            }
        }
    };
    let state = match value.as_deref() {
        Some("") => PropertyValueState::Empty,
        Some(_) => PropertyValueState::Present,
        None => PropertyValueState::UnsupportedType,
    };
    let preserved = value.or_else(|| {
        (saturating_u64(value_bytes.len()).saturating_mul(2) <= limits.max_string_bytes)
            .then(|| hex_bytes(value_bytes))
    });
    Ok(CountedString {
        value: preserved,
        state,
        fully_interpreted,
        null_terminated: terminator.is_some(),
        code_page_missing,
    })
}

fn typed_value_truncated(property: PropertyDescriptor, _kind: &'static str) -> DecodeFailure {
    DecodeFailure::malformed(
        "legacy.typed_property_truncated",
        "OLE typed property ends inside its scalar value",
        property.offset,
    )
}

fn property_type_name(type_code: u16) -> String {
    match type_code {
        0x0000 => "VT_EMPTY".to_owned(),
        0x0001 => "VT_NULL".to_owned(),
        0x0002 => "VT_I2".to_owned(),
        0x0003 => "VT_I4".to_owned(),
        0x0004 => "VT_R4".to_owned(),
        0x0005 => "VT_R8".to_owned(),
        0x0008 => "VT_BSTR".to_owned(),
        0x000b => "VT_BOOL".to_owned(),
        0x0010 => "VT_I1".to_owned(),
        0x0011 => "VT_UI1".to_owned(),
        0x0012 => "VT_UI2".to_owned(),
        0x0013 => "VT_UI4".to_owned(),
        0x0014 => "VT_I8".to_owned(),
        0x0015 => "VT_UI8".to_owned(),
        0x0016 => "VT_INT".to_owned(),
        0x0017 => "VT_UINT".to_owned(),
        0x001e => "VT_LPSTR".to_owned(),
        0x001f => "VT_LPWSTR".to_owned(),
        0x0040 => "VT_FILETIME".to_owned(),
        0x0041 => "VT_BLOB".to_owned(),
        0x0047 => "VT_CF".to_owned(),
        value => format!("0x{value:04x}"),
    }
}

fn property_name(
    entry: &InventoryEntry,
    format: PropertySetFormat,
    identifier: u32,
    dictionary_name: Option<&String>,
) -> (String, ValueOrigin, Vec<String>) {
    if let Some(name) = standard_property_name(format, identifier) {
        return (
            name.to_owned(),
            ValueOrigin::Derived,
            vec![format!(
                "ms-oleps:{}:pid:{identifier}",
                property_set_format_name(format)
            )],
        );
    }
    if let Some(name) = dictionary_name {
        return (
            name.clone(),
            ValueOrigin::Source,
            evidence(entry, &format!("property_dictionary:pid:{identifier}")),
        );
    }
    (
        format!("PID_{identifier}"),
        ValueOrigin::Derived,
        evidence(entry, &format!("property_identifier:{identifier}")),
    )
}

fn standard_property_name(format: PropertySetFormat, identifier: u32) -> Option<&'static str> {
    match (format, identifier) {
        (
            PropertySetFormat::Summary
            | PropertySetFormat::DocumentSummary
            | PropertySetFormat::UserDefined,
            1,
        ) => Some("CodePage"),
        (PropertySetFormat::Summary, 2) => Some("Title"),
        (PropertySetFormat::Summary, 3) => Some("Subject"),
        (PropertySetFormat::Summary, 4) => Some("Author"),
        (PropertySetFormat::Summary, 5) => Some("Keywords"),
        (PropertySetFormat::Summary, 6) => Some("Comments"),
        (PropertySetFormat::Summary, 7) => Some("Template"),
        (PropertySetFormat::Summary, 8) => Some("LastAuthor"),
        (PropertySetFormat::Summary, 9) => Some("RevisionNumber"),
        (PropertySetFormat::Summary, 10) => Some("EditTime"),
        (PropertySetFormat::Summary, 11) => Some("LastPrintedTime"),
        (PropertySetFormat::Summary, 12) => Some("CreatedTime"),
        (PropertySetFormat::Summary, 13) => Some("LastSavedTime"),
        (PropertySetFormat::Summary, 14) => Some("PageCount"),
        (PropertySetFormat::Summary, 15) => Some("WordCount"),
        (PropertySetFormat::Summary, 16) => Some("CharacterCount"),
        (PropertySetFormat::Summary, 17) => Some("Thumbnail"),
        (PropertySetFormat::Summary, 18) => Some("ApplicationName"),
        (PropertySetFormat::Summary, 19) => Some("DocumentSecurity"),
        (PropertySetFormat::DocumentSummary, 2) => Some("Category"),
        (PropertySetFormat::DocumentSummary, 3) => Some("PresentationTarget"),
        (PropertySetFormat::DocumentSummary, 4) => Some("ByteCount"),
        (PropertySetFormat::DocumentSummary, 5) => Some("LineCount"),
        (PropertySetFormat::DocumentSummary, 6) => Some("ParagraphCount"),
        (PropertySetFormat::DocumentSummary, 7) => Some("SlideCount"),
        (PropertySetFormat::DocumentSummary, 8) => Some("NoteCount"),
        (PropertySetFormat::DocumentSummary, 9) => Some("HiddenSlideCount"),
        (PropertySetFormat::DocumentSummary, 10) => Some("MultimediaClipCount"),
        (PropertySetFormat::DocumentSummary, 11) => Some("ScaleCrop"),
        (PropertySetFormat::DocumentSummary, 14) => Some("Manager"),
        (PropertySetFormat::DocumentSummary, 15) => Some("Company"),
        (PropertySetFormat::DocumentSummary, 16) => Some("LinksDirty"),
        (PropertySetFormat::DocumentSummary, 17) => Some("CharacterCountWithSpaces"),
        (PropertySetFormat::DocumentSummary, 19) => Some("SharedDocument"),
        (PropertySetFormat::DocumentSummary, 20) => Some("LinkBase"),
        (PropertySetFormat::DocumentSummary, 22) => Some("HyperlinksChanged"),
        (PropertySetFormat::DocumentSummary, 23) => Some("ApplicationVersion"),
        (PropertySetFormat::DocumentSummary, 26) => Some("ContentType"),
        (PropertySetFormat::DocumentSummary, 27) => Some("ContentStatus"),
        (PropertySetFormat::DocumentSummary, 28) => Some("Language"),
        (PropertySetFormat::DocumentSummary, 29) => Some("DocumentVersion"),
        _ => None,
    }
}

const fn property_set_format_name(format: PropertySetFormat) -> &'static str {
    match format {
        PropertySetFormat::Summary => "summary_information",
        PropertySetFormat::DocumentSummary => "document_summary_information",
        PropertySetFormat::UserDefined => "user_defined_properties",
        PropertySetFormat::Unknown => "unknown",
    }
}

struct LegacyConfigurationRecord {
    index: i64,
    name: String,
    parent_index: Option<i64>,
    record_offset: usize,
    name_offset: usize,
    parent_offset: usize,
    decoded_offset: usize,
}

fn decode_configuration_header(
    entry: &InventoryEntry,
    bytes: &[u8],
    internal_version: Option<u64>,
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    if !matches!(internal_version, Some(2_200 | 7_000)) {
        builder.mark(&entry.id, SemanticClass::Uninterpreted);
        builder.diagnostics.push(
            Diagnostic::new(
                "legacy.configuration_manager_version_unsupported",
                DiagnosticSeverity::Info,
                DiagnosticKind::Unsupported,
                "legacy configuration-header decoding is not enabled for this internal version",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail(
                "internal_version",
                internal_version.map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
            ),
        );
        return;
    }
    let record = match parse_legacy_configuration_header(bytes, limits) {
        Ok(record) => record,
        Err(failure) => {
            if failure.kind == DiagnosticKind::Fatal {
                builder.rejected = true;
            }
            builder.mark(
                &entry.id,
                if failure.kind == DiagnosticKind::Malformed {
                    SemanticClass::Malformed
                } else {
                    SemanticClass::Uninterpreted
                },
            );
            builder.diagnostics.push(failure_diagnostic(entry, failure));
            return;
        }
    };

    let candidate = builder.ensure_config(
        record.index,
        evidence(
            entry,
            &format!(
                "configuration_manager.record@decoded:{}:configuration_id",
                record.record_offset
            ),
        ),
    );
    candidate.name = Some(SourceValue::new(
        record.name,
        ValueOrigin::Source,
        evidence(
            entry,
            &format!(
                "configuration_manager.record@decoded:{}:name",
                record.name_offset
            ),
        ),
    ));
    if let Some(parent_index) = record.parent_index {
        candidate.parent_index = Some(SourceValue::new(
            parent_index,
            ValueOrigin::Source,
            evidence(
                entry,
                &format!(
                    "configuration_manager.record@decoded:{}:parent_id",
                    record.parent_offset
                ),
            ),
        ));
    }
    if record.decoded_offset < bytes.len() {
        builder.diagnostics.push(
            Diagnostic::new(
                "legacy.configuration_manager_trailing_bytes_preserved",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "bytes after the supported configuration-header prefix remain uninterpreted",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail("decoded_offset", record.decoded_offset.to_string())
            .with_detail(
                "remaining_bytes",
                bytes
                    .len()
                    .saturating_sub(record.decoded_offset)
                    .to_string(),
            ),
        );
    }
    builder.mark(&entry.id, SemanticClass::PartiallyInterpreted);
}

fn parse_legacy_configuration_header(
    bytes: &[u8],
    limits: &ResourceLimits,
) -> Result<LegacyConfigurationRecord, DecodeFailure> {
    let mut cursor = 0_usize;
    read_archive_class(bytes, &mut cursor, "dmConfigMgrHeader_c", limits)?;
    let count_offset = cursor;
    let raw_count = read_cursor_u16(bytes, &mut cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends before its record count",
            count_offset,
        )
    })?;
    let count = if raw_count == u16::MAX {
        read_cursor_u32(bytes, &mut cursor).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.configuration_manager_truncated",
                "legacy configuration header ends inside its extended record count",
                cursor,
            )
        })?
    } else {
        u32::from(raw_count)
    };
    if u64::from(count) > limits.max_stream_count {
        return Err(DecodeFailure::limit(
            "limit.configuration_count",
            "legacy configuration count exceeds the configured stream limit",
            count_offset,
        ));
    }
    if count != 1 {
        return Err(DecodeFailure::unsupported(
            "legacy.configuration_manager_count_unsupported",
            "legacy configuration headers with multiple records are outside the supported profile",
            count_offset,
        ));
    }

    let record_offset = cursor;
    read_archive_class(bytes, &mut cursor, "dmConfigHeader_c", limits)?;
    read_required_cursor_u32(bytes, &mut cursor)?;
    let name_offset = cursor;
    let name = read_archive_string(bytes, &mut cursor, limits)?;
    let raw_index = read_required_cursor_u32(bytes, &mut cursor)?;
    read_required_cursor_u32(bytes, &mut cursor)?;
    let repeated_name = read_archive_string(bytes, &mut cursor, limits)?;
    if repeated_name != name {
        return Err(DecodeFailure::unsupported(
            "legacy.configuration_manager_name_layout_unsupported",
            "the observed legacy configuration-name fields do not agree",
            record_offset,
        ));
    }
    let parent_offset = cursor;
    let raw_parent = read_required_cursor_u32(bytes, &mut cursor)?;
    Ok(LegacyConfigurationRecord {
        index: i64::from(raw_index),
        name,
        parent_index: (raw_parent != u32::MAX).then_some(i64::from(raw_parent)),
        record_offset,
        name_offset,
        parent_offset,
        decoded_offset: cursor,
    })
}

fn read_archive_class(
    bytes: &[u8],
    cursor: &mut usize,
    expected_name: &str,
    limits: &ResourceLimits,
) -> Result<(), DecodeFailure> {
    let tag_offset = *cursor;
    let tag = read_cursor_u16(bytes, cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends before an archive class tag",
            tag_offset,
        )
    })?;
    if tag != u16::MAX {
        return Err(DecodeFailure::unsupported(
            "legacy.configuration_manager_class_reference_unsupported",
            "legacy configuration archive class references are outside the supported profile",
            tag_offset,
        ));
    }
    read_cursor_u16(bytes, cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends inside an archive class schema",
            *cursor,
        )
    })?;
    let length_offset = *cursor;
    let name_length = usize::from(read_cursor_u16(bytes, cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends before an archive class name length",
            length_offset,
        )
    })?);
    if saturating_u64(name_length) > limits.max_string_bytes {
        return Err(DecodeFailure::limit(
            "limit.string_bytes",
            "legacy archive class name exceeds the configured string limit",
            length_offset,
        ));
    }
    let name_offset = *cursor;
    let end = name_offset.checked_add(name_length).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "legacy archive class-name range overflows the host numeric range",
            name_offset,
        )
    })?;
    let raw = bytes.get(name_offset..end).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends inside an archive class name",
            name_offset,
        )
    })?;
    let name = std::str::from_utf8(raw).map_err(|_| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_class_name_invalid",
            "legacy archive class name is not ASCII-compatible text",
            name_offset,
        )
    })?;
    *cursor = end;
    if name != expected_name {
        return Err(DecodeFailure::unsupported(
            "legacy.configuration_manager_class_unsupported",
            "legacy archive class is outside the supported profile",
            name_offset,
        ));
    }
    Ok(())
}

fn read_archive_string(
    bytes: &[u8],
    cursor: &mut usize,
    limits: &ResourceLimits,
) -> Result<String, DecodeFailure> {
    let marker_offset = *cursor;
    let first = read_cursor_u8(bytes, cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends before an archive string",
            marker_offset,
        )
    })?;
    if first == 0 {
        return Ok(String::new());
    }
    if first != u8::MAX {
        return read_archive_ascii(bytes, cursor, usize::from(first), limits);
    }
    let second = read_cursor_u8(bytes, cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends inside an archive string marker",
            marker_offset,
        )
    })?;
    let third = read_cursor_u8(bytes, cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends inside an archive string marker",
            marker_offset,
        )
    })?;
    if second != 0xfe || third != u8::MAX {
        return Err(DecodeFailure::unsupported(
            "legacy.configuration_manager_string_encoding_unsupported",
            "legacy archive string encoding is outside the supported profile",
            marker_offset,
        ));
    }
    let length = usize::from(read_cursor_u8(bytes, cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends before a UTF-16 string length",
            marker_offset,
        )
    })?);
    if matches!(length, 0xfe | 0xff) {
        return Err(DecodeFailure::unsupported(
            "legacy.configuration_manager_string_length_unsupported",
            "extended legacy archive string lengths are outside the supported profile",
            marker_offset,
        ));
    }
    let byte_len = length.checked_mul(2).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "legacy UTF-16 string byte length overflows the host numeric range",
            marker_offset,
        )
    })?;
    if saturating_u64(byte_len) > limits.max_string_bytes {
        return Err(DecodeFailure::limit(
            "limit.string_bytes",
            "legacy configuration name exceeds the configured string limit",
            marker_offset,
        ));
    }
    let value_offset = *cursor;
    let end = value_offset.checked_add(byte_len).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "legacy UTF-16 string range overflows the host numeric range",
            value_offset,
        )
    })?;
    let raw = bytes.get(value_offset..end).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends inside UTF-16 text",
            value_offset,
        )
    })?;
    let units = raw
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let value = String::from_utf16(&units).map_err(|_| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_string_encoding_invalid",
            "legacy configuration name is not valid UTF-16LE",
            value_offset,
        )
    })?;
    *cursor = end;
    Ok(value)
}

fn read_archive_ascii(
    bytes: &[u8],
    cursor: &mut usize,
    length: usize,
    limits: &ResourceLimits,
) -> Result<String, DecodeFailure> {
    if saturating_u64(length) > limits.max_string_bytes {
        return Err(DecodeFailure::limit(
            "limit.string_bytes",
            "legacy configuration name exceeds the configured string limit",
            *cursor,
        ));
    }
    let value_offset = *cursor;
    let end = value_offset.checked_add(length).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "legacy archive string range overflows the host numeric range",
            value_offset,
        )
    })?;
    let raw = bytes.get(value_offset..end).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends inside ASCII-compatible text",
            value_offset,
        )
    })?;
    let value = std::str::from_utf8(raw).map_err(|_| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_string_encoding_invalid",
            "legacy configuration name is not valid UTF-8",
            value_offset,
        )
    })?;
    *cursor = end;
    Ok(value.to_owned())
}

fn read_required_cursor_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, DecodeFailure> {
    let offset = *cursor;
    read_cursor_u32(bytes, cursor).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.configuration_manager_truncated",
            "legacy configuration header ends inside a configuration record",
            offset,
        )
    })
}

fn decode_document_preview(
    entry: &InventoryEntry,
    bytes: &[u8],
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    match decode_preview_resource(entry, bytes, limits) {
        Ok((resource, class, trailing)) => {
            if trailing > 0 {
                builder.diagnostics.push(
                    Diagnostic::new(
                        "legacy.preview_trailing_bytes_preserved",
                        DiagnosticSeverity::Info,
                        DiagnosticKind::Preserved,
                        "bytes after the validated preview carrier remain uninterpreted",
                    )
                    .in_stream(entry.path.as_deref().unwrap_or_default())
                    .with_detail("remaining_bytes", trailing.to_string()),
                );
            }
            select_preview(
                &mut builder.preview,
                resource,
                entry,
                &mut builder.diagnostics,
            );
            builder.mark(&entry.id, class);
        }
        Err(failure) => {
            builder.mark(
                &entry.id,
                if failure.kind == DiagnosticKind::Malformed {
                    SemanticClass::Malformed
                } else {
                    SemanticClass::Uninterpreted
                },
            );
            if failure.kind == DiagnosticKind::Fatal {
                builder.rejected = true;
            }
            builder.diagnostics.push(failure_diagnostic(entry, failure));
        }
    }
}

fn decode_configuration_preview(
    entry: &InventoryEntry,
    bytes: &[u8],
    index: i64,
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    match decode_preview_resource(entry, bytes, limits) {
        Ok((resource, class, trailing)) => {
            if trailing > 0 {
                builder.diagnostics.push(
                    Diagnostic::new(
                        "legacy.preview_trailing_bytes_preserved",
                        DiagnosticSeverity::Info,
                        DiagnosticKind::Preserved,
                        "bytes after the validated configuration preview remain uninterpreted",
                    )
                    .in_stream(entry.path.as_deref().unwrap_or_default())
                    .with_detail("configuration_index", index.to_string())
                    .with_detail("remaining_bytes", trailing.to_string()),
                );
            }
            if let Some(current) = builder.config_previews.get_mut(&index) {
                if preview_rank(resource.kind) > preview_rank(current.kind) {
                    *current = resource;
                }
            } else {
                builder.config_previews.insert(index, resource);
            }
            builder.mark(&entry.id, class);
        }
        Err(failure) => {
            builder.mark(
                &entry.id,
                if failure.kind == DiagnosticKind::Malformed {
                    SemanticClass::Malformed
                } else {
                    SemanticClass::Uninterpreted
                },
            );
            if failure.kind == DiagnosticKind::Fatal {
                builder.rejected = true;
            }
            builder.diagnostics.push(failure_diagnostic(entry, failure));
        }
    }
}

fn select_preview(
    current: &mut Option<BinaryResource>,
    candidate: BinaryResource,
    entry: &InventoryEntry,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(selected) = current {
        let replace = preview_rank(candidate.kind) > preview_rank(selected.kind);
        diagnostics.push(
            Diagnostic::new(
                "legacy.multiple_document_previews",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "multiple legacy preview carriers were found; the preferred validated carrier was selected",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail("replacement_selected", replace.to_string()),
        );
        if replace {
            *selected = candidate;
        }
    } else {
        *current = Some(candidate);
    }
}

const fn preview_rank(kind: BinaryResourceKind) -> u8 {
    match kind {
        BinaryResourceKind::PreviewPng => 2,
        BinaryResourceKind::PreviewDib => 1,
    }
}

fn decode_preview_resource(
    entry: &InventoryEntry,
    bytes: &[u8],
    limits: &ResourceLimits,
) -> Result<(BinaryResource, SemanticClass, usize), DecodeFailure> {
    if bytes.starts_with(PNG_SIGNATURE) {
        let end = validate_png(bytes, limits)?;
        return Ok((
            BinaryResource {
                kind: BinaryResourceKind::PreviewPng,
                entry_id: entry.id.clone(),
                stream_path: entry.path.clone().unwrap_or_default(),
                decoded_offset: 0,
                byte_len: saturating_u64(end),
                sha256: sha256_hex(&bytes[..end]),
                media_type: "image/png".to_owned(),
            },
            SemanticClass::PartiallyInterpreted,
            bytes.len().saturating_sub(end),
        ));
    }
    decode_dib_preview(entry, bytes)
}

#[allow(clippy::too_many_lines)]
fn decode_dib_preview(
    entry: &InventoryEntry,
    bytes: &[u8],
) -> Result<(BinaryResource, SemanticClass, usize), DecodeFailure> {
    let declared = usize::try_from(u32_at(bytes, 0).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.preview_dib_truncated",
            "legacy DIB preview ends before its payload length",
            0,
        )
    })?)
    .map_err(|_| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "legacy DIB preview length exceeds the host numeric range",
            0,
        )
    })?;
    let end = 4_usize.checked_add(declared).ok_or_else(|| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "legacy DIB preview range overflows the host numeric range",
            0,
        )
    })?;
    if end > bytes.len() {
        return Err(DecodeFailure::malformed(
            "legacy.preview_dib_truncated",
            "legacy DIB preview payload extends beyond its decoded stream",
            4,
        ));
    }
    if declared < 40 {
        return Err(DecodeFailure::unsupported(
            "legacy.preview_dib_header_unsupported",
            "legacy preview uses a DIB header outside the supported profile",
            4,
        ));
    }
    let header_size = usize::try_from(u32_at(bytes, 4).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.preview_dib_truncated",
            "legacy DIB preview ends before its bitmap header size",
            4,
        )
    })?)
    .map_err(|_| {
        DecodeFailure::limit(
            "limit.numeric_range",
            "legacy DIB header size exceeds the host numeric range",
            4,
        )
    })?;
    if header_size < 40 || header_size > declared {
        return Err(DecodeFailure::unsupported(
            "legacy.preview_dib_header_unsupported",
            "legacy preview uses a DIB header outside the supported profile",
            4,
        ));
    }
    let width = i32_at(bytes, 8).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.preview_dib_truncated",
            "legacy DIB preview ends inside its dimensions",
            8,
        )
    })?;
    let height = i32_at(bytes, 12).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.preview_dib_truncated",
            "legacy DIB preview ends inside its dimensions",
            12,
        )
    })?;
    let planes = u16_at(bytes, 16).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.preview_dib_truncated",
            "legacy DIB preview ends before its plane count",
            16,
        )
    })?;
    let bits_per_pixel = u16_at(bytes, 18).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.preview_dib_truncated",
            "legacy DIB preview ends before its bit depth",
            18,
        )
    })?;
    let compression = u32_at(bytes, 20).ok_or_else(|| {
        DecodeFailure::malformed(
            "legacy.preview_dib_truncated",
            "legacy DIB preview ends before its compression mode",
            20,
        )
    })?;
    if width == 0 || height == 0 || planes != 1 {
        return Err(DecodeFailure::malformed(
            "legacy.preview_dib_header_invalid",
            "legacy DIB preview has invalid dimensions or plane count",
            8,
        ));
    }
    if !matches!(bits_per_pixel, 1 | 4 | 8 | 16 | 24 | 32) || compression > 5 {
        return Err(DecodeFailure::unsupported(
            "legacy.preview_dib_encoding_unsupported",
            "legacy DIB preview encoding is outside the supported carrier profile",
            18,
        ));
    }
    Ok((
        BinaryResource {
            kind: BinaryResourceKind::PreviewDib,
            entry_id: entry.id.clone(),
            stream_path: entry.path.clone().unwrap_or_default(),
            decoded_offset: 4,
            byte_len: saturating_u64(declared),
            sha256: sha256_hex(&bytes[4..end]),
            media_type: "image/x-ms-bmp-dib".to_owned(),
        },
        SemanticClass::PartiallyInterpreted,
        bytes.len().saturating_sub(end),
    ))
}

#[allow(clippy::too_many_lines)]
fn validate_png(bytes: &[u8], limits: &ResourceLimits) -> Result<usize, DecodeFailure> {
    let mut cursor = PNG_SIGNATURE.len();
    let mut chunks = 0_u64;
    let mut saw_header = false;
    let mut saw_data = false;
    loop {
        chunks = chunks.saturating_add(1);
        if chunks > limits.max_stream_count {
            return Err(DecodeFailure::limit(
                "limit.png_chunks",
                "legacy preview PNG chunk count exceeds the configured stream limit",
                cursor,
            ));
        }
        let length = usize::try_from(u32_be(bytes, cursor).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.preview_png_truncated",
                "legacy preview PNG ends inside a chunk length",
                cursor,
            )
        })?)
        .map_err(|_| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "legacy preview PNG chunk length exceeds the host numeric range",
                cursor,
            )
        })?;
        let chunk_type_start = cursor.checked_add(4).ok_or_else(|| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "legacy preview PNG chunk offset overflows the host numeric range",
                cursor,
            )
        })?;
        let data_start = chunk_type_start.checked_add(4).ok_or_else(|| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "legacy preview PNG data offset overflows the host numeric range",
                cursor,
            )
        })?;
        let data_end = data_start.checked_add(length).ok_or_else(|| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "legacy preview PNG chunk range overflows the host numeric range",
                cursor,
            )
        })?;
        let chunk_end = data_end.checked_add(4).ok_or_else(|| {
            DecodeFailure::limit(
                "limit.numeric_range",
                "legacy preview PNG CRC range overflows the host numeric range",
                cursor,
            )
        })?;
        let chunk_type = bytes.get(chunk_type_start..data_start).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.preview_png_truncated",
                "legacy preview PNG ends inside a chunk type",
                chunk_type_start,
            )
        })?;
        let chunk_data = bytes.get(data_start..data_end).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.preview_png_truncated",
                "legacy preview PNG ends inside chunk data",
                data_start,
            )
        })?;
        let expected_crc = u32_be(bytes, data_end).ok_or_else(|| {
            DecodeFailure::malformed(
                "legacy.preview_png_truncated",
                "legacy preview PNG ends inside a chunk checksum",
                data_end,
            )
        })?;
        let mut crc = Crc32::new();
        crc.update(chunk_type);
        crc.update(chunk_data);
        if crc.finalize() != expected_crc {
            return Err(DecodeFailure::malformed(
                "legacy.preview_png_crc_mismatch",
                "legacy preview PNG chunk checksum does not match",
                data_end,
            ));
        }
        if chunk_type == b"IHDR" {
            if saw_header || length != 13 || cursor != PNG_SIGNATURE.len() {
                return Err(DecodeFailure::malformed(
                    "legacy.preview_png_header_invalid",
                    "legacy preview PNG does not contain one leading IHDR chunk",
                    chunk_type_start,
                ));
            }
            saw_header = true;
        } else if chunk_type == b"IDAT" {
            saw_data = true;
        } else if chunk_type == b"IEND" {
            if length != 0 || !saw_header || !saw_data {
                return Err(DecodeFailure::malformed(
                    "legacy.preview_png_end_invalid",
                    "legacy preview PNG ends without required image structure",
                    chunk_type_start,
                ));
            }
            return Ok(chunk_end);
        }
        cursor = chunk_end;
    }
}

fn failure_diagnostic(entry: &InventoryEntry, failure: DecodeFailure) -> Diagnostic {
    Diagnostic::new(
        failure.code,
        failure.severity,
        failure.kind,
        failure.message,
    )
    .in_stream(entry.path.clone().unwrap_or_default())
    .with_detail("entry_id", &entry.id)
    .with_detail("decoded_offset", failure.offset.to_string())
}

fn align_four(value: usize) -> Option<usize> {
    value.checked_add(3).map(|aligned| aligned & !3)
}

fn u16_at(data: &[u8], offset: usize) -> Option<u16> {
    let raw: [u8; 2] = data.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(raw))
}

fn i16_at(data: &[u8], offset: usize) -> Option<i16> {
    let raw: [u8; 2] = data.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
    Some(i16::from_le_bytes(raw))
}

fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
    let raw: [u8; 4] = data.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(raw))
}

fn i32_at(data: &[u8], offset: usize) -> Option<i32> {
    let raw: [u8; 4] = data.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(i32::from_le_bytes(raw))
}

fn u64_at(data: &[u8], offset: usize) -> Option<u64> {
    let raw: [u8; 8] = data.get(offset..offset.checked_add(8)?)?.try_into().ok()?;
    Some(u64::from_le_bytes(raw))
}

fn i64_at(data: &[u8], offset: usize) -> Option<i64> {
    let raw: [u8; 8] = data.get(offset..offset.checked_add(8)?)?.try_into().ok()?;
    Some(i64::from_le_bytes(raw))
}

fn u32_be(data: &[u8], offset: usize) -> Option<u32> {
    let raw: [u8; 4] = data.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(raw))
}

fn read_cursor_u8(data: &[u8], cursor: &mut usize) -> Option<u8> {
    let value = *data.get(*cursor)?;
    *cursor = cursor.checked_add(1)?;
    Some(value)
}

fn read_cursor_u16(data: &[u8], cursor: &mut usize) -> Option<u16> {
    let value = u16_at(data, *cursor)?;
    *cursor = cursor.checked_add(2)?;
    Some(value)
}

fn read_cursor_u32(data: &[u8], cursor: &mut usize) -> Option<u32> {
    let value = u32_at(data, *cursor)?;
    *cursor = cursor.checked_add(4)?;
    Some(value)
}

fn guid_packet_hex(value: &[u8; 16]) -> String {
    hex_bytes(value)
}

fn hex_bytes(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(value.len().saturating_mul(2));
    for byte in value {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn sha256_hex(data: &[u8]) -> String {
    hex_bytes(&Sha256::digest(data))
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
