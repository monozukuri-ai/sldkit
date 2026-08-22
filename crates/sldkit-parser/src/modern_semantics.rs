use std::collections::{BTreeMap, BTreeSet};

use crc32fast::Hasher as Crc32;
use roxmltree::{Document, Node, ParsingOptions};
use sha2::{Digest, Sha256};
use sldkit_container::decode_selected_bytes;
use sldkit_core::{
    AssemblyComponent, BinaryResource, BinaryResourceKind, Configuration, ContainerInventory,
    CustomProperty, Diagnostic, DiagnosticKind, DiagnosticSeverity, DocumentKind,
    DocumentReference, DrawingSheet, DrawingView, InventoryEntry, InventoryEntryState,
    MassProperties, PropertyKind, PropertyScope, PropertyValueState, RecordOffsetBasis,
    ReferenceKind, ResourceLimits, SemanticCoverage, SourceValue, UnknownRecord, ValueOrigin,
};

const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub(crate) struct ModernFacts {
    pub document_kind: SourceValue<DocumentKind>,
    pub internal_version: Option<SourceValue<u64>>,
    pub configurations: Vec<Configuration>,
    pub properties: Vec<CustomProperty>,
    pub references: Vec<DocumentReference>,
    pub preview: Option<BinaryResource>,
    pub sheets: Vec<DrawingSheet>,
    pub unknown_records: Vec<UnknownRecord>,
    pub diagnostics: Vec<Diagnostic>,
    pub semantic_coverage: SemanticCoverage,
    pub rejected: bool,
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
    index_origin: Option<ValueOrigin>,
    index_evidence: Vec<String>,
    names: Vec<(u8, SourceValue<String>)>,
    parent_names: Vec<SourceValue<String>>,
    parent_indices: Vec<SourceValue<i64>>,
}

#[derive(Default)]
struct Builder {
    kind_candidates: Vec<SourceValue<DocumentKind>>,
    version_candidates: BTreeMap<u64, Vec<String>>,
    configs: BTreeMap<i64, ConfigCandidate>,
    properties: Vec<CustomProperty>,
    components: BTreeMap<i64, Vec<AssemblyComponent>>,
    references: Vec<DocumentReference>,
    preview: Option<BinaryResource>,
    config_previews: BTreeMap<i64, BinaryResource>,
    sheets: Vec<DrawingSheet>,
    sheet_names: Vec<SourceValue<String>>,
    sheet_previews: BTreeMap<usize, BinaryResource>,
    diagnostics: Vec<Diagnostic>,
    classes: BTreeMap<String, SemanticClass>,
    rejected: bool,
}

pub(crate) fn decode(
    data: &[u8],
    inventory: &ContainerInventory,
    filename_kind: &SourceValue<DocumentKind>,
    limits: &ResourceLimits,
) -> ModernFacts {
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
    collect_config_path_candidates(inventory, &mut builder);
    let unambiguous_internal_version = if builder.version_candidates.len() == 1 {
        builder.version_candidates.keys().next().copied()
    } else {
        None
    };

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
                    "modern.selected_stream_unavailable",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "a decoded inventory entry was unavailable during the semantic scan",
                )
                .in_stream(path)
                .with_detail("entry_id", &entry.id),
            );
            continue;
        };
        decode_stream(
            entry,
            bytes,
            unambiguous_internal_version,
            limits,
            &mut builder,
        );
    }

    let document_kind = builder.resolve_document_kind(filename_kind);
    let internal_version = builder.resolve_version();
    let mut configurations = builder.finish_configurations(document_kind.value);
    builder.attach_configuration_names(&configurations);
    attach_configuration_resources(
        &mut configurations,
        &mut builder.config_previews,
        &mut builder.components,
    );
    attach_sheet_previews(
        &mut builder.sheets,
        &builder.sheet_names,
        &mut builder.sheet_previews,
    );
    let (semantic_coverage, unknown_records) = semantic_accounting(inventory, &builder.classes);

    ModernFacts {
        document_kind,
        internal_version,
        configurations,
        properties: builder.properties,
        references: builder.references,
        preview: builder.preview,
        sheets: builder.sheets,
        unknown_records,
        diagnostics: builder.diagnostics,
        semantic_coverage,
        rejected: builder.rejected,
    }
}

fn is_selected_stream(path: &str) -> bool {
    matches!(
        path,
        "docProps/custom.xml"
            | "docProps/core.xml"
            | "docProps/ISolidWorksInformation.xml"
            | "swXmlContents/Features"
            | "swXmlContents/COMPINSTANCETREE"
            | "swXmlContents/KeyWords"
            | "Contents/CMgrHdr2"
            | "PreviewPNG"
            | "SheetPreviews/SheetNames"
    ) || config_index_from_property_path(path).is_some()
        || config_index_from_preview_path(path).is_some()
        || sheet_index_from_preview_path(path).is_some()
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
        "docProps/custom.xml" => {
            decode_property_xml(
                entry,
                bytes,
                PropertyKind::Custom,
                PropertyScope::Global,
                None,
                limits,
                builder,
            );
        }
        "docProps/core.xml" => decode_core_xml(entry, bytes, limits, builder),
        "docProps/ISolidWorksInformation.xml" => {
            decode_property_xml(
                entry,
                bytes,
                PropertyKind::System,
                PropertyScope::Global,
                None,
                limits,
                builder,
            );
        }
        "swXmlContents/Features" => decode_model_xml(entry, bytes, false, limits, builder),
        "swXmlContents/COMPINSTANCETREE" => {
            decode_model_xml(entry, bytes, true, limits, builder);
        }
        "swXmlContents/KeyWords" => decode_keywords_xml(entry, bytes, limits, builder),
        "Contents/CMgrHdr2" => {
            decode_configuration_manager_header(entry, bytes, internal_version, limits, builder);
        }
        "PreviewPNG" => {
            if let Some((resource, class)) = decode_png_resource(entry, bytes, limits, builder) {
                if builder.preview.is_some() {
                    builder.diagnostics.push(
                        Diagnostic::new(
                            "modern.duplicate_document_preview",
                            DiagnosticSeverity::Warning,
                            DiagnosticKind::Preserved,
                            "multiple decoded PreviewPNG streams were found; the first is selected",
                        )
                        .in_stream(path),
                    );
                } else {
                    builder.preview = Some(resource);
                }
                builder.mark(&entry.id, class);
            }
        }
        "SheetPreviews/SheetNames" => match decode_sheet_names(entry, bytes, limits, builder) {
            Some((names, class)) => {
                builder.sheet_names = names;
                builder.mark(&entry.id, class);
            }
            None => builder.mark(&entry.id, SemanticClass::Malformed),
        },
        _ => {
            if let Some(index) = config_index_from_property_path(path) {
                builder.ensure_config(index, evidence(entry, "stream_path"));
                decode_property_xml(
                    entry,
                    bytes,
                    PropertyKind::Custom,
                    PropertyScope::Configuration,
                    Some(index),
                    limits,
                    builder,
                );
            } else if let Some(index) = config_index_from_preview_path(path) {
                if let Some((resource, class)) = decode_png_resource(entry, bytes, limits, builder)
                {
                    if builder.config_previews.insert(index, resource).is_some() {
                        builder.diagnostics.push(
                            Diagnostic::new(
                                "modern.duplicate_configuration_preview",
                                DiagnosticSeverity::Warning,
                                DiagnosticKind::Preserved,
                                "multiple previews target the same configuration index",
                            )
                            .in_stream(path)
                            .with_detail("configuration_index", index.to_string()),
                        );
                    }
                    builder.ensure_config(index, evidence(entry, "stream_path"));
                    builder.mark(&entry.id, class);
                }
            } else if let Some(index) = sheet_index_from_preview_path(path)
                && let Some((resource, class)) = decode_png_resource(entry, bytes, limits, builder)
            {
                builder.sheet_previews.insert(index, resource);
                builder.mark(&entry.id, class);
            }
        }
    }
}

// This deliberately narrow profile is limited to independently controlled
// observations of version 19_000. Generic CArchive object framing is documented
// by Microsoft TN002; other versions remain unsupported.
const CONFIGURATION_MANAGER_OBSERVED_VERSION: u64 = 19_000;

struct ConfigurationManagerRecord {
    index: i64,
    name: String,
    parent_index: Option<i64>,
    record_offset: usize,
    name_offset: usize,
    parent_offset: usize,
}

struct ConfigurationManagerDecodeError {
    code: &'static str,
    message: &'static str,
    offset: usize,
    severity: DiagnosticSeverity,
    kind: DiagnosticKind,
}

impl ConfigurationManagerDecodeError {
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

fn decode_configuration_manager_header(
    entry: &InventoryEntry,
    bytes: &[u8],
    internal_version: Option<u64>,
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    if internal_version != Some(CONFIGURATION_MANAGER_OBSERVED_VERSION) {
        builder.mark(&entry.id, SemanticClass::Uninterpreted);
        builder.diagnostics.push(
            Diagnostic::new(
                "modern.configuration_manager_version_unsupported",
                DiagnosticSeverity::Info,
                DiagnosticKind::Unsupported,
                "configuration-manager header layout is not enabled for this internal version",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail(
                "internal_version",
                internal_version.map_or_else(|| "unknown".to_owned(), |value| value.to_string()),
            ),
        );
        return;
    }

    let (records, decoded_offset) = match parse_configuration_manager_header_19000(bytes, limits) {
        Ok(value) => value,
        Err(error) => {
            if error.kind == DiagnosticKind::Fatal {
                builder.rejected = true;
            }
            builder.mark(
                &entry.id,
                if error.kind == DiagnosticKind::Malformed {
                    SemanticClass::Malformed
                } else {
                    SemanticClass::Uninterpreted
                },
            );
            builder.diagnostics.push(
                Diagnostic::new(error.code, error.severity, error.kind, error.message)
                    .in_stream(entry.path.as_deref().unwrap_or_default())
                    .with_detail("entry_id", &entry.id)
                    .with_detail("decoded_offset", error.offset.to_string()),
            );
            return;
        }
    };

    for record in records {
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
        candidate.names.push((
            0,
            SourceValue::new(
                record.name,
                ValueOrigin::Source,
                evidence(
                    entry,
                    &format!(
                        "configuration_manager.record@decoded:{}:name",
                        record.name_offset
                    ),
                ),
            ),
        ));
        if let Some(parent_index) = record.parent_index {
            candidate.parent_indices.push(SourceValue::new(
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
    }

    if decoded_offset < bytes.len() {
        builder.diagnostics.push(
            Diagnostic::new(
                "modern.configuration_manager_trailing_bytes_preserved",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "bytes after the observed configuration records remain uninterpreted",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default())
            .with_detail("decoded_offset", decoded_offset.to_string())
            .with_detail(
                "remaining_bytes",
                bytes.len().saturating_sub(decoded_offset).to_string(),
            ),
        );
    }
    builder.mark(&entry.id, SemanticClass::PartiallyInterpreted);
}

fn parse_configuration_manager_header_19000(
    bytes: &[u8],
    limits: &ResourceLimits,
) -> Result<(Vec<ConfigurationManagerRecord>, usize), ConfigurationManagerDecodeError> {
    let mut cursor = 0_usize;
    read_configuration_archive_class(bytes, &mut cursor, "dmConfigMgrHeader_c", limits)?;
    let count_offset = cursor;
    let raw_count = read_u16(bytes, &mut cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends before its record count",
            count_offset,
        )
    })?;
    let count = if raw_count == u16::MAX {
        let offset = cursor;
        read_u32_le(bytes, &mut cursor).ok_or_else(|| {
            ConfigurationManagerDecodeError::malformed(
                "modern.configuration_manager_truncated",
                "configuration-manager header ends inside its extended record count",
                offset,
            )
        })?
    } else {
        u32::from(raw_count)
    };
    if u64::from(count) > limits.max_stream_count {
        return Err(ConfigurationManagerDecodeError::limit(
            "limit.configuration_count",
            "configuration-manager record count exceeds the configured stream limit",
            count_offset,
        ));
    }
    let capacity = usize::try_from(count).map_err(|_| {
        ConfigurationManagerDecodeError::limit(
            "limit.configuration_count",
            "configuration-manager record count exceeds the host numeric range",
            count_offset,
        )
    })?;
    let mut records = Vec::with_capacity(capacity);
    for _ in 0..count {
        let record_offset = cursor;
        read_configuration_archive_class(bytes, &mut cursor, "dmConfigHeader_c", limits)?;
        read_required_u32(bytes, &mut cursor, "record flags")?;
        let name_offset = cursor;
        let name = read_configuration_archive_string(bytes, &mut cursor, limits)?;
        let raw_index = read_required_u32(bytes, &mut cursor, "configuration ID")?;
        let index = i64::from(raw_index);
        read_required_u32(bytes, &mut cursor, "configuration stamp")?;
        let repeated_name = read_configuration_archive_string(bytes, &mut cursor, limits)?;
        if repeated_name != name {
            return Err(ConfigurationManagerDecodeError::unsupported(
                "modern.configuration_manager_name_layout_unsupported",
                "the observed configuration-manager name fields do not agree",
                record_offset,
            ));
        }
        let parent_offset = cursor;
        let raw_parent = read_required_u32(bytes, &mut cursor, "parent configuration ID")?;
        read_required_u32(bytes, &mut cursor, "opaque configuration field")?;
        let _ = read_configuration_archive_string(bytes, &mut cursor, limits)?;
        let _ = read_configuration_archive_string(bytes, &mut cursor, limits)?;
        for _ in 0..4 {
            read_required_u32(bytes, &mut cursor, "opaque configuration field")?;
        }
        records.push(ConfigurationManagerRecord {
            index,
            name,
            parent_index: (raw_parent != u32::MAX).then_some(i64::from(raw_parent)),
            record_offset,
            name_offset,
            parent_offset,
        });
    }
    Ok((records, cursor))
}

fn read_configuration_archive_class(
    bytes: &[u8],
    cursor: &mut usize,
    expected_name: &str,
    limits: &ResourceLimits,
) -> Result<(), ConfigurationManagerDecodeError> {
    let tag_offset = *cursor;
    let tag = read_u16(bytes, cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends before an archive class tag",
            tag_offset,
        )
    })?;
    if tag != u16::MAX {
        if tag & 0x8000 != 0 {
            return Ok(());
        }
        return Err(ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_object_tag_invalid",
            "an archive object record does not contain a serializable class tag",
            tag_offset,
        ));
    }

    read_u16(bytes, cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends inside an archive class schema",
            *cursor,
        )
    })?;
    let length_offset = *cursor;
    let name_length = usize::from(read_u16(bytes, cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends before an archive class name length",
            length_offset,
        )
    })?);
    if u64::try_from(name_length).unwrap_or(u64::MAX) > limits.max_string_bytes {
        return Err(ConfigurationManagerDecodeError::limit(
            "limit.string_bytes",
            "configuration-manager class name exceeds the configured string limit",
            length_offset,
        ));
    }
    let name_offset = *cursor;
    let end = cursor.checked_add(name_length).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_numeric_range",
            "configuration-manager class-name range overflows the host numeric range",
            name_offset,
        )
    })?;
    let raw_name = bytes.get(name_offset..end).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends inside an archive class name",
            name_offset,
        )
    })?;
    let name = std::str::from_utf8(raw_name).map_err(|_| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_class_name_invalid",
            "configuration-manager archive class name is not ASCII-compatible text",
            name_offset,
        )
    })?;
    *cursor = end;
    if name != expected_name {
        return Err(ConfigurationManagerDecodeError::unsupported(
            "modern.configuration_manager_class_unsupported",
            "configuration-manager archive class is outside the observed profile",
            name_offset,
        ));
    }
    Ok(())
}

fn read_configuration_archive_string(
    bytes: &[u8],
    cursor: &mut usize,
    limits: &ResourceLimits,
) -> Result<String, ConfigurationManagerDecodeError> {
    let marker_offset = *cursor;
    let first = read_u8(bytes, cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends before an archive string",
            marker_offset,
        )
    })?;
    if first == 0 {
        return Ok(String::new());
    }
    if first != u8::MAX {
        return read_configuration_archive_ascii(bytes, cursor, usize::from(first), limits);
    }

    let second = read_u8(bytes, cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends inside an archive string marker",
            marker_offset,
        )
    })?;
    let third = read_u8(bytes, cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends inside an archive string marker",
            marker_offset,
        )
    })?;
    if second != 0xfe || third != u8::MAX {
        return Err(ConfigurationManagerDecodeError::unsupported(
            "modern.configuration_manager_string_encoding_unsupported",
            "archive string encoding is outside the controlled internal-version 19000 profile",
            marker_offset,
        ));
    }
    let raw_length = read_u8(bytes, cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends before a UTF-16 string length",
            marker_offset,
        )
    })?;
    if matches!(raw_length, 0xfe | 0xff) {
        return Err(ConfigurationManagerDecodeError::unsupported(
            "modern.configuration_manager_string_length_unsupported",
            "extended archive string lengths are outside the controlled profile",
            marker_offset,
        ));
    }
    let length = usize::from(raw_length);
    let byte_len = length.checked_mul(2).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_numeric_range",
            "configuration-manager UTF-16 byte length overflows the host numeric range",
            marker_offset,
        )
    })?;
    if u64::try_from(byte_len).unwrap_or(u64::MAX) > limits.max_string_bytes {
        return Err(ConfigurationManagerDecodeError::limit(
            "limit.string_bytes",
            "configuration-manager string exceeds the configured string limit",
            marker_offset,
        ));
    }
    let value_offset = *cursor;
    let end = cursor.checked_add(byte_len).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_numeric_range",
            "configuration-manager string range overflows the host numeric range",
            value_offset,
        )
    })?;
    let raw = bytes.get(value_offset..end).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends inside UTF-16 text",
            value_offset,
        )
    })?;
    let units = raw
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let value = String::from_utf16(&units).map_err(|_| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_string_encoding_invalid",
            "configuration-manager string is not valid UTF-16LE",
            value_offset,
        )
    })?;
    *cursor = end;
    Ok(value)
}

fn read_configuration_archive_ascii(
    bytes: &[u8],
    cursor: &mut usize,
    length: usize,
    limits: &ResourceLimits,
) -> Result<String, ConfigurationManagerDecodeError> {
    let value_offset = *cursor;
    if u64::try_from(length).unwrap_or(u64::MAX) > limits.max_string_bytes {
        return Err(ConfigurationManagerDecodeError::limit(
            "limit.string_bytes",
            "configuration-manager string exceeds the configured string limit",
            value_offset,
        ));
    }
    let end = cursor.checked_add(length).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_numeric_range",
            "configuration-manager string range overflows the host numeric range",
            value_offset,
        )
    })?;
    let raw = bytes.get(value_offset..end).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends inside ASCII text",
            value_offset,
        )
    })?;
    let value = std::str::from_utf8(raw).map_err(|_| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_string_encoding_invalid",
            "configuration-manager string is not valid ASCII-compatible text",
            value_offset,
        )
    })?;
    *cursor = end;
    Ok(value.to_owned())
}

fn read_required_u32(
    bytes: &[u8],
    cursor: &mut usize,
    _field: &'static str,
) -> Result<u32, ConfigurationManagerDecodeError> {
    let offset = *cursor;
    read_u32_le(bytes, cursor).ok_or_else(|| {
        ConfigurationManagerDecodeError::malformed(
            "modern.configuration_manager_truncated",
            "configuration-manager header ends inside a configuration record",
            offset,
        )
    })
}

#[allow(clippy::too_many_lines)]
fn decode_property_xml(
    entry: &InventoryEntry,
    bytes: &[u8],
    kind: PropertyKind,
    scope: PropertyScope,
    configuration_index: Option<i64>,
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    let path = entry.path.as_deref().unwrap_or_default();
    let Some(document) = parse_xml(entry, bytes, limits, builder) else {
        builder.mark(&entry.id, SemanticClass::Malformed);
        return;
    };

    let mut decoded_any = false;
    for section in document
        .descendants()
        .filter(|node| node.has_tag_name("propertySection"))
    {
        if kind == PropertyKind::Custom
            && section.attribute("name") != Some("UserDefinedProperties")
        {
            continue;
        }
        for property in section
            .children()
            .filter(|node| node.has_tag_name("property"))
        {
            let Some(name) = property.attribute("name") else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            decoded_any = true;
            let value_node = property.children().find(Node::is_element);
            let value_type = value_node.map(|node| node.tag_name().name().to_owned());
            let raw_value = value_node.map(|node| {
                SourceValue::new(
                    node.text().unwrap_or_default().to_owned(),
                    ValueOrigin::Source,
                    evidence(entry, &format!("property:{name}:value")),
                )
            });
            let value_state = match (&value_type, &raw_value) {
                (Some(value_type), Some(value)) if !is_supported_property_type(value_type) => {
                    builder.diagnostics.push(
                        Diagnostic::new(
                            "modern.property_type_unsupported",
                            DiagnosticSeverity::Warning,
                            DiagnosticKind::Unsupported,
                            "property value type is preserved but not semantically decoded",
                        )
                        .in_stream(path)
                        .with_detail("property", name)
                        .with_detail("value_type", value_type),
                    );
                    let _ = value;
                    PropertyValueState::UnsupportedType
                }
                (Some(_), Some(value)) if value.value.is_empty() => PropertyValueState::Empty,
                (Some(_), Some(_)) => PropertyValueState::Present,
                (None, _) | (Some(_), None) => PropertyValueState::Missing,
            };
            if value_state == PropertyValueState::Missing {
                builder.diagnostics.push(
                    Diagnostic::new(
                        "modern.property_value_missing",
                        DiagnosticSeverity::Info,
                        DiagnosticKind::Preserved,
                        "property exists but has no value element",
                    )
                    .in_stream(path)
                    .with_detail("property", name),
                );
            }
            let pid = property
                .attribute("pid")
                .and_then(|value| value.parse::<u32>().ok());
            builder.properties.push(CustomProperty {
                name: SourceValue::new(
                    name.to_owned(),
                    ValueOrigin::Source,
                    evidence(entry, &format!("property:{name}:name")),
                ),
                raw_value,
                value_type,
                value_state,
                kind,
                scope,
                configuration: None,
                configuration_index,
                stream_path: path.to_owned(),
                pid,
            });
        }
    }

    if !decoded_any {
        builder.diagnostics.push(
            Diagnostic::new(
                "modern.property_stream_has_no_named_values",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "property stream contains no named properties in the selected section",
            )
            .in_stream(path),
        );
    }
    builder.mark(&entry.id, SemanticClass::PartiallyInterpreted);
}

fn decode_core_xml(
    entry: &InventoryEntry,
    bytes: &[u8],
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    let Some(document) = parse_xml(entry, bytes, limits, builder) else {
        builder.mark(&entry.id, SemanticClass::Malformed);
        return;
    };
    for node in document.root_element().children().filter(Node::is_element) {
        let name = node.tag_name().name().to_owned();
        let value = node.text().unwrap_or_default().to_owned();
        builder.properties.push(CustomProperty {
            name: SourceValue::new(
                name.clone(),
                ValueOrigin::Source,
                evidence(entry, &format!("core:{name}:name")),
            ),
            raw_value: Some(SourceValue::new(
                value.clone(),
                ValueOrigin::Source,
                evidence(entry, &format!("core:{name}:value")),
            )),
            value_type: Some("xml_text".to_owned()),
            value_state: if value.is_empty() {
                PropertyValueState::Empty
            } else {
                PropertyValueState::Present
            },
            kind: PropertyKind::Core,
            scope: PropertyScope::Global,
            configuration: None,
            configuration_index: None,
            stream_path: entry.path.clone().unwrap_or_default(),
            pid: None,
        });
    }
    // The source text for every direct child is retained, but namespace and
    // richer Dublin Core typing are outside the supported semantic profile.
    builder.mark(&entry.id, SemanticClass::PartiallyInterpreted);
}

fn decode_model_xml(
    entry: &InventoryEntry,
    bytes: &[u8],
    assembly_tree: bool,
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    let Some(document) = parse_xml(entry, bytes, limits, builder) else {
        builder.mark(&entry.id, SemanticClass::Malformed);
        return;
    };
    collect_document_kind(entry, &document, builder);
    collect_xml_configurations(entry, &document, builder);
    if assembly_tree {
        collect_components(entry, &document, builder);
    } else {
        collect_external_feature_references(entry, &document, builder);
    }
    builder.mark(&entry.id, SemanticClass::PartiallyInterpreted);
}

fn decode_keywords_xml(
    entry: &InventoryEntry,
    bytes: &[u8],
    limits: &ResourceLimits,
    builder: &mut Builder,
) {
    let Some(document) = parse_xml(entry, bytes, limits, builder) else {
        builder.mark(&entry.id, SemanticClass::Malformed);
        return;
    };
    collect_xml_configurations(entry, &document, builder);
    let root = document.root_element();
    let mut sheets = Vec::new();
    for sheet_node in root.children().filter(|node| node.has_tag_name("Sheet")) {
        let source_type = sheet_node.attribute("Type");
        let has_direct_view = sheet_node.children().any(|node| node.has_tag_name("View"));
        if source_type != Some("Sheet") && !has_direct_view {
            continue;
        }
        if source_type != Some("Sheet") {
            let mut diagnostic = Diagnostic::new(
                "modern.drawing_sheet_type_inferred_from_views",
                DiagnosticSeverity::Info,
                DiagnosticKind::Inferred,
                "drawing sheet semantics were inferred from direct View children",
            )
            .in_stream(entry.path.as_deref().unwrap_or_default());
            if let Some(source_type) = source_type {
                diagnostic = diagnostic.with_detail("source_type", source_type);
            }
            builder.diagnostics.push(diagnostic);
        }
        let sheet_name = source_attribute(entry, sheet_node, "Name", "sheet");
        let source_id = sheet_node.attribute("id").map(str::to_owned);
        let mut views = Vec::new();
        for view_node in sheet_node
            .children()
            .filter(|node| node.has_tag_name("View"))
        {
            let name = source_attribute(entry, view_node, "Name", "drawing_view");
            let referenced_configuration =
                source_attribute(entry, view_node, "Description", "drawing_view");
            let referenced_document = view_node.text().map(str::trim).map(|value| {
                SourceValue::new(
                    value.to_owned(),
                    ValueOrigin::Source,
                    evidence(entry, "drawing_view:text"),
                )
            });
            let source_view_id = view_node.attribute("id").map(str::to_owned);
            builder.references.push(DocumentReference {
                kind: ReferenceKind::DrawingView,
                source_name: name.clone(),
                stored_path: referenced_document.clone(),
                resolved_path: None,
                document_kind: None,
                configuration: referenced_configuration.clone(),
                configuration_index: None,
            });
            views.push(DrawingView {
                source_id: source_view_id,
                name,
                referenced_document,
                referenced_configuration,
            });
        }
        sheets.push(DrawingSheet {
            source_id,
            name: sheet_name,
            preview: None,
            views,
        });
    }
    if !sheets.is_empty() {
        builder.kind_candidates.push(SourceValue::new(
            DocumentKind::Drawing,
            ValueOrigin::Source,
            evidence(entry, "Keywords/Sheet[@Type='Sheet' or View]"),
        ));
        builder.sheets = sheets;
    }
    builder.mark(&entry.id, SemanticClass::PartiallyInterpreted);
}

fn parse_xml<'a>(
    entry: &InventoryEntry,
    bytes: &'a [u8],
    limits: &ResourceLimits,
    builder: &mut Builder,
) -> Option<Document<'a>> {
    let path = entry.path.as_deref().unwrap_or_default();
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limits.max_xml_stream_bytes {
        builder.rejected = true;
        builder.diagnostics.push(
            Diagnostic::new(
                "limit.xml_stream_bytes",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "decoded XML stream exceeds the configured semantic parser limit",
            )
            .in_stream(path)
            .with_detail("actual_bytes", bytes.len().to_string())
            .with_detail("limit_bytes", limits.max_xml_stream_bytes.to_string()),
        );
        return None;
    }
    let Some(start) = bytes.iter().position(|value| *value == b'<') else {
        builder.diagnostics.push(xml_diagnostic(
            entry,
            "modern.xml_root_missing",
            "decoded XML stream has no document start",
        ));
        return None;
    };
    let Ok(text) = std::str::from_utf8(&bytes[start..]) else {
        builder.diagnostics.push(xml_diagnostic(
            entry,
            "modern.xml_encoding_invalid",
            "decoded XML stream is not valid UTF-8",
        ));
        return None;
    };
    let options = ParsingOptions {
        allow_dtd: false,
        nodes_limit: limits.max_xml_nodes,
        entity_resolver: None,
    };
    match Document::parse_with_options(text, options) {
        Ok(document) => Some(document),
        Err(error) => {
            if matches!(error, roxmltree::Error::NodesLimitReached) {
                builder.rejected = true;
                builder.diagnostics.push(
                    Diagnostic::new(
                        "limit.xml_nodes",
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Fatal,
                        "decoded XML stream exceeds the configured node limit",
                    )
                    .in_stream(path)
                    .with_detail("limit", limits.max_xml_nodes.to_string()),
                );
            } else {
                builder.diagnostics.push(
                    xml_diagnostic(
                        entry,
                        "modern.xml_malformed",
                        "decoded XML stream is malformed or uses a disallowed construct",
                    )
                    .with_detail("error", error.to_string()),
                );
            }
            None
        }
    }
}

fn collect_document_kind(entry: &InventoryEntry, document: &Document<'_>, builder: &mut Builder) {
    let own_file = document
        .descendants()
        .find(|node| node.has_tag_name("swHeader"))
        .and_then(|header| header.children().find(|node| node.has_tag_name("swFile")));
    let Some(own_file) = own_file else {
        return;
    };
    let Some(raw_kind) = own_file.attribute("swDocType") else {
        return;
    };
    let kind = match raw_kind.to_ascii_uppercase().as_str() {
        "PART" => DocumentKind::Part,
        "ASSEMBLY" => DocumentKind::Assembly,
        "DRAWING" => DocumentKind::Drawing,
        _ => {
            builder.diagnostics.push(
                Diagnostic::new(
                    "modern.document_kind_unknown",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "internal document type is not recognized",
                )
                .in_stream(entry.path.clone().unwrap_or_default())
                .with_detail("raw_kind", raw_kind),
            );
            DocumentKind::Unknown
        }
    };
    builder.kind_candidates.push(SourceValue::new(
        kind,
        ValueOrigin::Source,
        evidence(entry, "swHeader/swFile[0]@swDocType"),
    ));
}

fn collect_xml_configurations(
    entry: &InventoryEntry,
    document: &Document<'_>,
    builder: &mut Builder,
) {
    for node in document.descendants().filter(Node::is_element) {
        let (index_attribute, name_attribute, priority) = match node.tag_name().name() {
            "swConfiguration" => ("swID", "swName", 0),
            "Configuration" if node.attribute("Type") == Some("ConfigurationManager") => {
                ("id", "Name", 1)
            }
            _ => continue,
        };
        let Some(raw_index) = node.attribute(index_attribute) else {
            continue;
        };
        let Ok(index) = raw_index.parse::<i64>() else {
            builder.diagnostics.push(
                Diagnostic::new(
                    "modern.configuration_index_malformed",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "configuration index is not a signed decimal integer",
                )
                .in_stream(entry.path.clone().unwrap_or_default())
                .with_detail("raw_index", raw_index),
            );
            continue;
        };
        let name = node.attribute(name_attribute).map(|name| {
            (
                priority,
                SourceValue::new(
                    name.to_owned(),
                    ValueOrigin::Source,
                    evidence(entry, name_attribute),
                ),
            )
        });
        let mut parent_names = Vec::new();
        for attribute in ["swParentConfigurationName", "swParentName", "ParentName"] {
            if let Some(value) = node.attribute(attribute) {
                parent_names.push(SourceValue::new(
                    value.to_owned(),
                    ValueOrigin::Source,
                    evidence(entry, attribute),
                ));
            }
        }
        let mut parent_indices = Vec::new();
        for attribute in ["swParentConfigurationId", "swParentID", "ParentId"] {
            if let Some(value) = node.attribute(attribute) {
                match value.parse::<i64>() {
                    Ok(parent) => parent_indices.push(SourceValue::new(
                        parent,
                        ValueOrigin::Source,
                        evidence(entry, attribute),
                    )),
                    Err(_) => builder.diagnostics.push(
                        Diagnostic::new(
                            "modern.configuration_parent_index_malformed",
                            DiagnosticSeverity::Error,
                            DiagnosticKind::Malformed,
                            "configuration parent index is not a signed decimal integer",
                        )
                        .in_stream(entry.path.clone().unwrap_or_default())
                        .with_detail("raw_index", value),
                    ),
                }
            }
        }
        let candidate = builder.ensure_config(index, evidence(entry, index_attribute));
        if let Some(name) = name {
            candidate.names.push(name);
        }
        candidate.parent_names.extend(parent_names);
        candidate.parent_indices.extend(parent_indices);
    }
}

#[allow(clippy::too_many_lines)]
fn collect_components(entry: &InventoryEntry, document: &Document<'_>, builder: &mut Builder) {
    let root = document.root_element();
    let Some(header) = root.children().find(|node| node.has_tag_name("swHeader")) else {
        return;
    };
    let mut files: BTreeMap<String, (Option<String>, Option<DocumentKind>)> = BTreeMap::new();
    let mut fallback_own_file_id = None;
    for file in header.children().filter(|node| node.has_tag_name("swFile")) {
        let Some(id) = file.attribute("id") else {
            continue;
        };
        if fallback_own_file_id.is_none() {
            fallback_own_file_id = Some(id.to_owned());
        }
        let kind = file.attribute("swDocType").and_then(parse_document_kind);
        files.insert(
            id.to_owned(),
            (file.attribute("swPath").map(str::to_owned), kind),
        );
    }
    let Some(model_list) = root
        .children()
        .find(|node| node.has_tag_name("swModelList"))
    else {
        return;
    };
    let models = model_list
        .children()
        .filter(|node| node.has_tag_name("swModel"))
        .collect::<Vec<_>>();
    let mut configuration_root_models: BTreeMap<i64, BTreeSet<String>> = BTreeMap::new();
    if let Some(configuration_list) = root
        .children()
        .find(|node| node.has_tag_name("swConfigurationList"))
    {
        for configuration in configuration_list
            .children()
            .filter(|node| node.has_tag_name("swConfiguration"))
        {
            let (Some(raw_index), Some(model_ref)) = (
                configuration.attribute("swID"),
                configuration.attribute("swModelRef"),
            ) else {
                continue;
            };
            let Ok(index) = raw_index.parse::<i64>() else {
                builder.diagnostics.push(
                    Diagnostic::new(
                        "modern.configuration_root_model_index_malformed",
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Malformed,
                        "configuration root-model mapping has a non-integer configuration ID",
                    )
                    .in_stream(entry.path.as_deref().unwrap_or_default())
                    .with_detail("raw_index", raw_index),
                );
                continue;
            };
            configuration_root_models
                .entry(index)
                .or_default()
                .insert(model_ref.to_owned());
        }
    }
    for (index, model_refs) in &configuration_root_models {
        if model_refs.len() > 1 {
            builder.diagnostics.push(
                Diagnostic::new(
                    "modern.configuration_root_model_ambiguous",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "configuration maps to multiple root assembly models",
                )
                .in_stream(entry.path.as_deref().unwrap_or_default())
                .with_detail("configuration_index", index.to_string())
                .with_detail(
                    "model_refs",
                    model_refs.iter().cloned().collect::<Vec<_>>().join("|"),
                ),
            );
        }
    }
    let mut model_files = BTreeMap::new();
    let mut model_configs = BTreeMap::new();
    for model in &models {
        let Some(id) = model.attribute("id") else {
            continue;
        };
        if let Some(file_ref) = model.attribute("swFileRef") {
            model_files.insert(id.to_owned(), file_ref.to_owned());
        }
        if let Some(configuration) = model.attribute("swConfigurationName") {
            model_configs.insert(id.to_owned(), configuration.to_owned());
        }
    }

    for model in models {
        let Some(raw_index) = model.attribute("swConfigurationId") else {
            continue;
        };
        let Ok(configuration_index) = raw_index.parse::<i64>() else {
            continue;
        };
        let is_root_model = configuration_root_models
            .get(&configuration_index)
            .map_or_else(
                || {
                    fallback_own_file_id.as_deref().is_none()
                        || model.attribute("swFileRef") == fallback_own_file_id.as_deref()
                },
                |model_refs| {
                    model_refs.len() == 1
                        && model
                            .attribute("id")
                            .is_some_and(|id| model_refs.contains(id))
                },
            );
        if !is_root_model {
            continue;
        }
        if configuration_root_models.contains_key(&configuration_index)
            && fallback_own_file_id.as_deref().is_some()
            && model.attribute("swFileRef") != fallback_own_file_id.as_deref()
        {
            builder.diagnostics.push(
                Diagnostic::new(
                    "modern.configuration_root_model_relocated",
                    DiagnosticSeverity::Info,
                    DiagnosticKind::Preserved,
                    "configuration root model uses a self-file ID other than the first header entry",
                )
                .in_stream(entry.path.as_deref().unwrap_or_default())
                .with_detail("configuration_index", configuration_index.to_string())
                .with_detail(
                    "model_ref",
                    model.attribute("id").unwrap_or_default(),
                )
                .with_detail(
                    "selected_file_ref",
                    model.attribute("swFileRef").unwrap_or_default(),
                )
                .with_detail(
                    "first_file_ref",
                    fallback_own_file_id.as_deref().unwrap_or_default(),
                ),
            );
        }
        if !model
            .children()
            .any(|node| node.has_tag_name("swReference"))
        {
            continue;
        }
        builder.ensure_config(
            configuration_index,
            evidence(entry, "swModel@swConfigurationId"),
        );
        for reference in model
            .children()
            .filter(|node| node.has_tag_name("swReference"))
        {
            let model_ref = reference.attribute("swModelRef").map(str::to_owned);
            let file = model_ref
                .as_ref()
                .and_then(|value| model_files.get(value))
                .and_then(|value| files.get(value));
            let stored_path = file.and_then(|(path, _)| path.as_ref()).map(|path| {
                SourceValue::new(
                    path.clone(),
                    ValueOrigin::Source,
                    evidence(entry, "swHeader/swFile@swPath"),
                )
            });
            let document_kind = file.and_then(|(_, kind)| *kind).map(|kind| {
                SourceValue::new(
                    kind,
                    ValueOrigin::Source,
                    evidence(entry, "swHeader/swFile@swDocType"),
                )
            });
            let referenced_configuration = reference
                .attribute("swConfigurationName")
                .map(|value| {
                    SourceValue::new(
                        value.to_owned(),
                        ValueOrigin::Source,
                        evidence(entry, "swReference@swConfigurationName"),
                    )
                })
                .or_else(|| {
                    model_ref
                        .as_ref()
                        .and_then(|value| model_configs.get(value))
                        .map(|value| {
                            SourceValue::new(
                                value.clone(),
                                ValueOrigin::Derived,
                                evidence(entry, "swModel@swConfigurationName"),
                            )
                        })
                });
            let instance_name = source_attribute(entry, reference, "swName", "swReference");
            let component_reference =
                source_attribute(entry, reference, "swComponentReference", "swReference");
            let component = AssemblyComponent {
                configuration_index,
                instance_name: instance_name.clone(),
                stored_path: stored_path.clone(),
                document_kind: document_kind.clone(),
                referenced_configuration: referenced_configuration.clone(),
                component_reference,
                is_suppressed: source_bool_attribute(entry, reference, "swSuppressed", builder),
                is_hidden: source_bool_attribute(entry, reference, "swHidden", builder),
                exclude_from_bom: source_bool_attribute(
                    entry,
                    reference,
                    "swExcludeFromBOM",
                    builder,
                ),
                source_model_ref: model_ref,
                raw_attributes: reference
                    .attributes()
                    .map(|attribute| (attribute.name().to_owned(), attribute.value().to_owned()))
                    .collect(),
            };
            builder.references.push(DocumentReference {
                kind: ReferenceKind::AssemblyComponent,
                source_name: instance_name,
                stored_path,
                resolved_path: None,
                document_kind,
                configuration: referenced_configuration,
                configuration_index: Some(configuration_index),
            });
            builder
                .components
                .entry(configuration_index)
                .or_default()
                .push(component);
        }
    }
}

fn collect_external_feature_references(
    entry: &InventoryEntry,
    document: &Document<'_>,
    builder: &mut Builder,
) {
    let Some(header) = document
        .root_element()
        .children()
        .find(|node| node.has_tag_name("swHeader"))
    else {
        return;
    };
    for (index, file) in header
        .children()
        .filter(|node| node.has_tag_name("swFile"))
        .enumerate()
    {
        if index == 0 {
            continue;
        }
        let stored_path = source_attribute(entry, file, "swPath", "swFile");
        let document_kind = file
            .attribute("swDocType")
            .and_then(parse_document_kind)
            .map(|kind| {
                SourceValue::new(
                    kind,
                    ValueOrigin::Source,
                    evidence(entry, "swFile@swDocType"),
                )
            });
        builder.references.push(DocumentReference {
            kind: ReferenceKind::ExternalFeature,
            source_name: None,
            stored_path,
            resolved_path: None,
            document_kind,
            configuration: None,
            configuration_index: None,
        });
    }
}

#[allow(clippy::too_many_lines)]
fn decode_png_resource(
    entry: &InventoryEntry,
    bytes: &[u8],
    limits: &ResourceLimits,
    builder: &mut Builder,
) -> Option<(BinaryResource, SemanticClass)> {
    let path = entry.path.as_deref().unwrap_or_default();
    let Some(start) = bytes
        .windows(PNG_SIGNATURE.len())
        .position(|window| window == PNG_SIGNATURE)
    else {
        preview_failure(
            entry,
            builder,
            "modern.preview_png_missing_signature",
            "preview stream does not contain a PNG signature",
            0,
        );
        return None;
    };
    let mut cursor = start + PNG_SIGNATURE.len();
    let mut chunk_count = 0_u64;
    let mut saw_idat = false;
    let end = loop {
        chunk_count = chunk_count.saturating_add(1);
        if chunk_count > limits.max_stream_count {
            builder.rejected = true;
            builder.mark(&entry.id, SemanticClass::Malformed);
            builder.diagnostics.push(
                Diagnostic::new(
                    "limit.png_chunks",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "preview PNG chunk count exceeds the configured stream limit",
                )
                .in_stream(path),
            );
            return None;
        }
        let Some(raw_length) = u32_be(bytes, cursor) else {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_truncated",
                "preview PNG ends inside a chunk length",
                cursor,
            );
            return None;
        };
        let Ok(length) = usize::try_from(raw_length) else {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_numeric_range",
                "preview PNG chunk length is outside the host numeric range",
                cursor,
            );
            return None;
        };
        let Some(chunk_type_start) = cursor.checked_add(4) else {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_numeric_range",
                "preview PNG chunk offset overflows the host numeric range",
                cursor,
            );
            return None;
        };
        let Some(data_start) = chunk_type_start.checked_add(4) else {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_numeric_range",
                "preview PNG data offset overflows the host numeric range",
                cursor,
            );
            return None;
        };
        let Some(data_end) = data_start.checked_add(length) else {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_numeric_range",
                "preview PNG chunk end overflows the host numeric range",
                cursor,
            );
            return None;
        };
        let Some(chunk_end) = data_end.checked_add(4) else {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_numeric_range",
                "preview PNG CRC offset overflows the host numeric range",
                cursor,
            );
            return None;
        };
        let (Some(chunk_type), Some(chunk_data), Some(expected_crc)) = (
            bytes.get(chunk_type_start..data_start),
            bytes.get(data_start..data_end),
            u32_be(bytes, data_end),
        ) else {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_truncated",
                "preview PNG ends inside a chunk",
                cursor,
            );
            return None;
        };
        if chunk_count == 1 && (chunk_type != b"IHDR" || length != 13) {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_invalid_header",
                "preview PNG does not begin with a 13-byte IHDR chunk",
                cursor,
            );
            return None;
        }
        if chunk_type == b"IDAT" {
            saw_idat = true;
        }
        let mut crc = Crc32::new();
        crc.update(chunk_type);
        crc.update(chunk_data);
        if crc.finalize() != expected_crc {
            preview_failure(
                entry,
                builder,
                "modern.preview_png_crc_mismatch",
                "preview PNG chunk CRC-32 does not match",
                cursor,
            );
            return None;
        }
        cursor = chunk_end;
        if chunk_type == b"IEND" {
            if length != 0 || !saw_idat {
                preview_failure(
                    entry,
                    builder,
                    "modern.preview_png_invalid_iend",
                    "preview PNG IEND is invalid or no IDAT chunk precedes it",
                    cursor,
                );
                return None;
            }
            break chunk_end;
        }
    };
    let png = &bytes[start..end];
    let class = if start == 0 && end == bytes.len() {
        SemanticClass::FullyInterpreted
    } else {
        SemanticClass::PartiallyInterpreted
    };
    Some((
        BinaryResource {
            kind: BinaryResourceKind::PreviewPng,
            entry_id: entry.id.clone(),
            stream_path: path.to_owned(),
            decoded_offset: saturating_u64(start),
            byte_len: saturating_u64(png.len()),
            sha256: sha256_hex(png),
            media_type: "image/png".to_owned(),
        },
        class,
    ))
}

fn preview_failure(
    entry: &InventoryEntry,
    builder: &mut Builder,
    code: &str,
    message: &str,
    decoded_offset: usize,
) {
    builder.mark(&entry.id, SemanticClass::Malformed);
    builder
        .diagnostics
        .push(binary_diagnostic(entry, code, message, decoded_offset));
}

#[allow(clippy::too_many_lines)]
fn decode_sheet_names(
    entry: &InventoryEntry,
    bytes: &[u8],
    limits: &ResourceLimits,
    builder: &mut Builder,
) -> Option<(Vec<SourceValue<String>>, SemanticClass)> {
    let path = entry.path.as_deref().unwrap_or_default();
    let mut cursor = 0_usize;
    let Some(raw_count) = read_u16(bytes, &mut cursor) else {
        builder.diagnostics.push(binary_diagnostic(
            entry,
            "modern.sheet_names_truncated",
            "drawing sheet-name stream ends before its count",
            cursor,
        ));
        return None;
    };
    let count = usize::from(raw_count);
    if u64::try_from(count).unwrap_or(u64::MAX) > limits.max_stream_count {
        builder.rejected = true;
        builder.diagnostics.push(
            Diagnostic::new(
                "limit.sheet_count",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "drawing sheet-name count exceeds the configured stream limit",
            )
            .in_stream(path),
        );
        return None;
    }
    let mut names = Vec::with_capacity(count);
    for index in 0..count {
        if bytes.get(cursor..cursor.saturating_add(3)) != Some(&[0xff, 0xfe, 0xff]) {
            builder.diagnostics.push(binary_diagnostic(
                entry,
                "modern.sheet_names_malformed",
                "sheet-name entry marker is missing",
                cursor,
            ));
            return None;
        }
        cursor += 3;
        let Some(raw_length) = bytes.get(cursor) else {
            builder.diagnostics.push(binary_diagnostic(
                entry,
                "modern.sheet_names_truncated",
                "drawing sheet-name stream ends before a name length",
                cursor,
            ));
            return None;
        };
        let length = usize::from(*raw_length);
        cursor += 1;
        let Some(byte_len) = length.checked_mul(2) else {
            builder.diagnostics.push(binary_diagnostic(
                entry,
                "modern.sheet_names_numeric_range",
                "drawing sheet-name byte length overflows the host numeric range",
                cursor,
            ));
            return None;
        };
        if u64::try_from(byte_len).unwrap_or(u64::MAX) > limits.max_string_bytes {
            builder.rejected = true;
            builder.diagnostics.push(
                Diagnostic::new(
                    "limit.string_bytes",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "drawing sheet name exceeds the configured string limit",
                )
                .in_stream(path),
            );
            return None;
        }
        let Some(end) = cursor.checked_add(byte_len) else {
            builder.diagnostics.push(binary_diagnostic(
                entry,
                "modern.sheet_names_numeric_range",
                "drawing sheet-name end overflows the host numeric range",
                cursor,
            ));
            return None;
        };
        let Some(raw) = bytes.get(cursor..end) else {
            builder.diagnostics.push(binary_diagnostic(
                entry,
                "modern.sheet_names_truncated",
                "drawing sheet-name stream ends inside UTF-16LE text",
                cursor,
            ));
            return None;
        };
        let units = raw
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        let Ok(name) = String::from_utf16(&units) else {
            builder.diagnostics.push(binary_diagnostic(
                entry,
                "modern.sheet_name_encoding_invalid",
                "drawing sheet name is not valid UTF-16LE",
                cursor,
            ));
            return None;
        };
        names.push(SourceValue::new(
            name,
            ValueOrigin::Source,
            evidence(entry, &format!("sheet_names[{index}]")),
        ));
        cursor = end;
    }
    let trailing_is_padding = bytes[cursor..].iter().all(|value| *value == 0);
    let class = if trailing_is_padding {
        SemanticClass::FullyInterpreted
    } else {
        builder.diagnostics.push(
            Diagnostic::new(
                "modern.sheet_names_trailing_bytes",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "non-zero bytes remain after the decoded sheet-name list",
            )
            .in_stream(path)
            .with_detail("decoded_offset", cursor.to_string()),
        );
        SemanticClass::PartiallyInterpreted
    };
    Some((names, class))
}

fn collect_version_candidates(inventory: &ContainerInventory, builder: &mut Builder) {
    const PREFIX: &str = "_MO_VERSION_";
    for entry in &inventory.entries {
        let Some(path) = entry.path.as_deref() else {
            continue;
        };
        let Some(rest) = path.strip_prefix(PREFIX) else {
            continue;
        };
        let Some((digits, _)) = rest.split_once('/') else {
            continue;
        };
        match digits.parse::<u64>() {
            Ok(version) => {
                builder
                    .version_candidates
                    .entry(version)
                    .or_default()
                    .extend(evidence(entry, "stream_path"));
                if entry.state == InventoryEntryState::Decoded {
                    builder.mark(&entry.id, SemanticClass::PartiallyInterpreted);
                }
            }
            Err(_) => builder.diagnostics.push(
                Diagnostic::new(
                    "modern.internal_version_malformed",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "internal version path does not contain a decimal u64 value",
                )
                .in_stream(path),
            ),
        }
    }
}

fn collect_config_path_candidates(inventory: &ContainerInventory, builder: &mut Builder) {
    for entry in &inventory.entries {
        let Some(path) = entry.path.as_deref() else {
            continue;
        };
        if let Some(index) =
            config_index_from_property_path(path).or_else(|| config_index_from_preview_path(path))
        {
            builder.ensure_config(index, evidence(entry, "stream_path"));
        }
    }
}

impl Builder {
    fn mark(&mut self, entry_id: &str, class: SemanticClass) {
        let current = self
            .classes
            .entry(entry_id.to_owned())
            .or_insert(SemanticClass::Uninterpreted);
        *current = (*current).max(class);
    }

    fn ensure_config(&mut self, index: i64, mut evidence: Vec<String>) -> &mut ConfigCandidate {
        let candidate = self.configs.entry(index).or_default();
        candidate.index_origin = Some(ValueOrigin::Source);
        candidate.index_evidence.append(&mut evidence);
        candidate.index_evidence.sort();
        candidate.index_evidence.dedup();
        candidate
    }

    fn resolve_document_kind(
        &mut self,
        filename_kind: &SourceValue<DocumentKind>,
    ) -> SourceValue<DocumentKind> {
        let mut by_kind: BTreeMap<u8, (DocumentKind, Vec<String>)> = BTreeMap::new();
        for candidate in &self.kind_candidates {
            let rank = document_kind_rank(candidate.value);
            by_kind
                .entry(rank)
                .or_insert((candidate.value, Vec::new()))
                .1
                .extend(candidate.evidence.clone());
        }
        let source_kinds = by_kind
            .values()
            .filter(|(kind, _)| *kind != DocumentKind::Unknown)
            .collect::<Vec<_>>();
        let resolved = match source_kinds.len() {
            1 => SourceValue::new(
                source_kinds[0].0,
                ValueOrigin::Source,
                source_kinds[0].1.clone(),
            ),
            2.. => {
                self.diagnostics.push(Diagnostic::new(
                    "modern.document_kind_conflict",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "modern streams contain conflicting document-type evidence",
                ));
                SourceValue::new(
                    DocumentKind::Unknown,
                    ValueOrigin::Preserved,
                    source_kinds
                        .iter()
                        .flat_map(|(_, evidence)| evidence.iter().cloned())
                        .collect(),
                )
            }
            0 => filename_kind.clone(),
        };
        if resolved.origin == ValueOrigin::Source
            && filename_kind.value != DocumentKind::Unknown
            && resolved.value != filename_kind.value
        {
            self.diagnostics.push(
                Diagnostic::new(
                    "input.document_kind_mismatch",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "internal document type takes precedence over the filename extension",
                )
                .with_detail("content_kind", document_kind_name(resolved.value))
                .with_detail("extension_kind", document_kind_name(filename_kind.value)),
            );
        }
        resolved
    }

    fn resolve_version(&mut self) -> Option<SourceValue<u64>> {
        if self.version_candidates.len() == 1 {
            return self
                .version_candidates
                .iter()
                .next()
                .map(|(version, evidence)| {
                    SourceValue::new(*version, ValueOrigin::Source, evidence.clone())
                });
        }
        if self.version_candidates.len() > 1 {
            self.diagnostics.push(
                Diagnostic::new(
                    "modern.internal_version_conflict",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "multiple distinct internal version values are present in stream paths",
                )
                .with_detail(
                    "versions",
                    self.version_candidates
                        .keys()
                        .map(u64::to_string)
                        .collect::<Vec<_>>()
                        .join(","),
                ),
            );
        }
        None
    }

    #[allow(clippy::too_many_lines)]
    fn finish_configurations(&mut self, document_kind: DocumentKind) -> Vec<Configuration> {
        self.add_property_name_fallbacks();
        if self.configs.is_empty()
            && matches!(document_kind, DocumentKind::Part | DocumentKind::Assembly)
        {
            self.diagnostics.push(Diagnostic::new(
                "modern.configuration_index_inferred",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Inferred,
                "no configuration identity was decoded; index zero is retained as an inference",
            ));
            self.configs.insert(
                0,
                ConfigCandidate {
                    index_origin: Some(ValueOrigin::Inferred),
                    index_evidence: vec!["inference.single_configuration".to_owned()],
                    ..ConfigCandidate::default()
                },
            );
        }

        let all_indices = self.configs.keys().copied().collect::<BTreeSet<_>>();
        let all_names = self
            .configs
            .values()
            .flat_map(|candidate| candidate.names.iter().map(|(_, name)| name.value.clone()))
            .collect::<BTreeSet<_>>();
        let canonical_names = self
            .configs
            .iter()
            .filter_map(|(index, candidate)| {
                merge_ranked_names(candidate.names.clone())
                    .into_iter()
                    .next()
                    .map(|(_, name)| (*index, name))
            })
            .collect::<BTreeMap<_, _>>();
        let mut output = Vec::new();
        for (index, mut candidate) in std::mem::take(&mut self.configs) {
            candidate.names = merge_ranked_names(candidate.names);
            if candidate.names.len() > 1 {
                self.diagnostics.push(
                    Diagnostic::new(
                        "modern.configuration_name_conflict",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Preserved,
                        "configuration index has multiple distinct source names",
                    )
                    .with_detail("configuration_index", index.to_string())
                    .with_detail(
                        "names",
                        candidate
                            .names
                            .iter()
                            .map(|(_, name)| name.value.clone())
                            .collect::<Vec<_>>()
                            .join("|"),
                    ),
                );
            }
            let name = candidate.names.first().map(|(_, value)| value.clone());
            let alternate_names = candidate
                .names
                .into_iter()
                .skip(1)
                .map(|(_, value)| value)
                .collect();
            let parent_names = merge_sourced_values(candidate.parent_names);
            let parent_indices = merge_sourced_values(candidate.parent_indices);
            if parent_names.len() > 1 {
                self.diagnostics.push(
                    Diagnostic::new(
                        "modern.configuration_parent_name_conflict",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Preserved,
                        "configuration has multiple distinct source parent names",
                    )
                    .with_detail("configuration_index", index.to_string())
                    .with_detail(
                        "parent_names",
                        parent_names
                            .iter()
                            .map(|value| value.value.as_str())
                            .collect::<Vec<_>>()
                            .join("|"),
                    ),
                );
            }
            if parent_indices.len() > 1 {
                self.diagnostics.push(
                    Diagnostic::new(
                        "modern.configuration_parent_index_conflict",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Preserved,
                        "configuration has multiple distinct source parent indices",
                    )
                    .with_detail("configuration_index", index.to_string())
                    .with_detail(
                        "parent_indices",
                        parent_indices
                            .iter()
                            .map(|value| value.value.to_string())
                            .collect::<Vec<_>>()
                            .join("|"),
                    ),
                );
            }
            let mut parent_name = parent_names.into_iter().next();
            let parent_index = parent_indices.into_iter().next();
            if parent_name.is_none()
                && let Some(parent) = parent_index.as_ref()
                && let Some(resolved_name) = canonical_names.get(&parent.value)
            {
                let mut parent_evidence = parent.evidence.clone();
                parent_evidence.extend(resolved_name.evidence.iter().cloned());
                parent_evidence.push("derivation.configuration_parent_index_to_name".to_owned());
                parent_evidence.sort();
                parent_evidence.dedup();
                parent_name = Some(SourceValue::new(
                    resolved_name.value.clone(),
                    ValueOrigin::Derived,
                    parent_evidence,
                ));
            }
            if parent_name.as_ref().is_some_and(|parent| {
                !parent.value.is_empty() && !all_names.contains(&parent.value)
            }) {
                self.diagnostics.push(
                    Diagnostic::new(
                        "modern.configuration_parent_unresolved",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        "configuration parent name does not resolve within this document",
                    )
                    .with_detail("configuration_index", index.to_string())
                    .with_detail(
                        "parent_name",
                        parent_name
                            .as_ref()
                            .map_or(String::new(), |value| value.value.clone()),
                    ),
                );
            }
            if parent_index
                .as_ref()
                .is_some_and(|parent| !all_indices.contains(&parent.value))
            {
                self.diagnostics.push(
                    Diagnostic::new(
                        "modern.configuration_parent_index_unresolved",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Unresolved,
                        "configuration parent index does not resolve within this document",
                    )
                    .with_detail("configuration_index", index.to_string())
                    .with_detail(
                        "parent_index",
                        parent_index
                            .as_ref()
                            .map_or(String::new(), |value| value.value.to_string()),
                    ),
                );
            }
            output.push(Configuration {
                index: SourceValue::new(
                    index,
                    candidate.index_origin.unwrap_or(ValueOrigin::Inferred),
                    candidate.index_evidence,
                ),
                name,
                alternate_names,
                parent_name,
                parent_index,
                preview: None,
                mass_properties: self.mass_properties(index),
                components: Vec::new(),
            });
        }
        output
    }

    fn add_property_name_fallbacks(&mut self) {
        for property in &self.properties {
            if property.kind != PropertyKind::Custom
                || property.name.value != "Configuration"
                || property.value_state != PropertyValueState::Present
            {
                continue;
            }
            let (Some(index), Some(value)) =
                (property.configuration_index, property.raw_value.as_ref())
            else {
                continue;
            };
            self.configs.entry(index).or_default().names.push((
                2,
                SourceValue::new(
                    value.value.clone(),
                    ValueOrigin::Derived,
                    value.evidence.clone(),
                ),
            ));
        }
        let active_name = self
            .properties
            .iter()
            .find(|property| {
                property.kind == PropertyKind::System
                    && property.name.value == "SW-Configuration Name"
                    && property.value_state == PropertyValueState::Present
            })
            .and_then(|property| property.raw_value.as_ref())
            .cloned();
        if self.configs.len() == 1
            && self
                .configs
                .values()
                .next()
                .is_some_and(|candidate| candidate.names.is_empty())
            && let Some(active_name) = active_name
        {
            if let Some(candidate) = self.configs.values_mut().next() {
                candidate.names.push((
                    3,
                    SourceValue::new(
                        active_name.value,
                        ValueOrigin::Derived,
                        active_name.evidence,
                    ),
                ));
            }
            self.diagnostics.push(Diagnostic::new(
                "modern.configuration_name_derived",
                DiagnosticSeverity::Info,
                DiagnosticKind::Inferred,
                "the only unnamed configuration uses the active configuration system property",
            ));
        }
    }

    fn mass_properties(&mut self, index: i64) -> Option<MassProperties> {
        let target = format!("SW-MassProp-Config-{index}");
        let property = self.properties.iter().find(|property| {
            property.kind == PropertyKind::System
                && property.name.value == target
                && property.value_state == PropertyValueState::Present
        })?;
        let raw_value = property.raw_value.clone()?;
        let tokens = raw_value
            .value
            .split(',')
            .map(str::trim)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if tokens.len() < 12 || tokens.iter().any(|token| !is_finite_decimal(token)) {
            self.diagnostics.push(
                Diagnostic::new(
                    "modern.mass_properties_malformed",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "cached mass-property value does not contain at least twelve decimal numbers",
                )
                .in_stream(&property.stream_path)
                .with_detail("configuration_index", index.to_string()),
            );
            return None;
        }
        Some(MassProperties {
            raw_value,
            center_of_gravity: [tokens[0].clone(), tokens[1].clone(), tokens[2].clone()],
            volume: tokens[3].clone(),
            surface_area: tokens[4].clone(),
            mass: tokens[5].clone(),
            moments_of_inertia: [tokens[6].clone(), tokens[7].clone(), tokens[8].clone()],
            products_of_inertia: [tokens[9].clone(), tokens[10].clone(), tokens[11].clone()],
            additional_values: tokens.into_iter().skip(12).collect(),
        })
    }

    fn attach_configuration_names(&mut self, configurations: &[Configuration]) {
        let names = configurations
            .iter()
            .filter_map(|configuration| {
                configuration
                    .name
                    .as_ref()
                    .map(|name| (configuration.index.value, name.value.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        for property in &mut self.properties {
            property.configuration = property
                .configuration_index
                .and_then(|index| names.get(&index).cloned());
        }
    }
}

fn attach_configuration_resources(
    configurations: &mut [Configuration],
    previews: &mut BTreeMap<i64, BinaryResource>,
    components: &mut BTreeMap<i64, Vec<AssemblyComponent>>,
) {
    for configuration in configurations {
        configuration.preview = previews.remove(&configuration.index.value);
        configuration.components = components
            .remove(&configuration.index.value)
            .unwrap_or_default();
    }
}

fn attach_sheet_previews(
    sheets: &mut Vec<DrawingSheet>,
    names: &[SourceValue<String>],
    previews: &mut BTreeMap<usize, BinaryResource>,
) {
    if sheets.is_empty() {
        for (index, name) in names.iter().enumerate() {
            sheets.push(DrawingSheet {
                source_id: None,
                name: Some(name.clone()),
                preview: previews.remove(&index),
                views: Vec::new(),
            });
        }
        return;
    }
    let name_to_index = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.value.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    for sheet in sheets {
        let index = sheet
            .name
            .as_ref()
            .and_then(|name| name_to_index.get(name.value.as_str()))
            .copied();
        if let Some(index) = index {
            sheet.preview = previews.remove(&index);
        }
    }
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
        let reason = match class {
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
                Some("semantic.partially_interpreted_stream")
            }
            SemanticClass::Uninterpreted => {
                coverage.uninterpreted_streams = coverage.uninterpreted_streams.saturating_add(1);
                coverage.uninterpreted_bytes = coverage.uninterpreted_bytes.saturating_add(size);
                Some("semantic.stream_unsupported")
            }
            SemanticClass::Malformed => {
                coverage.malformed_streams = coverage.malformed_streams.saturating_add(1);
                coverage.malformed_bytes = coverage.malformed_bytes.saturating_add(size);
                Some("semantic.stream_malformed")
            }
        };
        if let Some(reason_code) = reason {
            unknown.push(UnknownRecord {
                entry_id: Some(entry.id.clone()),
                stream_path: entry.path.clone(),
                record_kind: entry
                    .attributes
                    .get("type_id")
                    .and_then(|value| value.parse::<u64>().ok()),
                offset_basis: RecordOffsetBasis::DecodedStream,
                offset: 0,
                length: size,
                sha256: entry.decoded_sha256.clone().unwrap_or_default(),
                reason_code: reason_code.to_owned(),
            });
        }
    }
    (coverage, unknown)
}

fn source_attribute(
    entry: &InventoryEntry,
    node: Node<'_, '_>,
    attribute: &str,
    context: &str,
) -> Option<SourceValue<String>> {
    node.attribute(attribute).map(|value| {
        SourceValue::new(
            value.to_owned(),
            ValueOrigin::Source,
            evidence(entry, &format!("{context}@{attribute}")),
        )
    })
}

fn source_bool_attribute(
    entry: &InventoryEntry,
    node: Node<'_, '_>,
    attribute: &str,
    builder: &mut Builder,
) -> Option<SourceValue<bool>> {
    let raw = node.attribute(attribute)?;
    let value = match raw.to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => true,
        "no" | "false" | "0" => false,
        _ => {
            builder.diagnostics.push(
                Diagnostic::new(
                    "modern.boolean_attribute_unsupported",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "boolean attribute uses an unrecognized source spelling",
                )
                .in_stream(entry.path.clone().unwrap_or_default())
                .with_detail("attribute", attribute)
                .with_detail("raw_value", raw),
            );
            return None;
        }
    };
    Some(SourceValue::new(
        value,
        ValueOrigin::Source,
        evidence(entry, &format!("swReference@{attribute}")),
    ))
}

fn parse_document_kind(value: &str) -> Option<DocumentKind> {
    match value.to_ascii_uppercase().as_str() {
        "PART" => Some(DocumentKind::Part),
        "ASSEMBLY" => Some(DocumentKind::Assembly),
        "DRAWING" => Some(DocumentKind::Drawing),
        _ => None,
    }
}

fn is_supported_property_type(value: &str) -> bool {
    matches!(
        value,
        "lpstr" | "lpwstr" | "i4" | "i2" | "ui4" | "ui2" | "r8" | "bool" | "date" | "filetime"
    )
}

fn config_index_from_property_path(path: &str) -> Option<i64> {
    let rest = path.strip_prefix("docProps/Config-")?;
    let digits = rest.strip_suffix("-Properties.xml")?;
    digits.parse().ok()
}

fn config_index_from_preview_path(path: &str) -> Option<i64> {
    let rest = path.strip_prefix("Config-")?;
    let digits = rest.strip_suffix("-PreviewPNG")?;
    digits.parse().ok()
}

fn sheet_index_from_preview_path(path: &str) -> Option<usize> {
    path.strip_prefix("Images/Sheet_")?.parse().ok()
}

fn merge_ranked_names(
    mut values: Vec<(u8, SourceValue<String>)>,
) -> Vec<(u8, SourceValue<String>)> {
    values.sort_by_key(|(priority, _)| *priority);
    let mut merged: Vec<(u8, SourceValue<String>)> = Vec::new();
    for (priority, mut value) in values {
        if let Some((existing_priority, existing)) = merged
            .iter_mut()
            .find(|(_, existing)| existing.value == value.value)
        {
            *existing_priority = (*existing_priority).min(priority);
            existing.evidence.append(&mut value.evidence);
            existing.evidence.sort();
            existing.evidence.dedup();
        } else {
            merged.push((priority, value));
        }
    }
    merged.sort_by_key(|(priority, _)| *priority);
    merged
}

fn merge_sourced_values<T: Eq>(values: Vec<SourceValue<T>>) -> Vec<SourceValue<T>> {
    let mut merged: Vec<SourceValue<T>> = Vec::new();
    for mut value in values {
        if let Some(existing) = merged
            .iter_mut()
            .find(|existing| existing.value == value.value)
        {
            existing.evidence.append(&mut value.evidence);
            existing.evidence.sort();
            existing.evidence.dedup();
        } else {
            merged.push(value);
        }
    }
    merged
}

fn is_finite_decimal(value: &str) -> bool {
    !value.is_empty() && value.parse::<f64>().is_ok_and(f64::is_finite)
}

fn evidence(entry: &InventoryEntry, selector: &str) -> Vec<String> {
    vec![format!("{}#{selector}", entry.id)]
}

fn xml_diagnostic(entry: &InventoryEntry, code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticKind::Malformed,
        message,
    )
    .in_stream(entry.path.clone().unwrap_or_default())
    .with_detail("entry_id", &entry.id)
}

fn binary_diagnostic(
    entry: &InventoryEntry,
    code: &str,
    message: &str,
    decoded_offset: usize,
) -> Diagnostic {
    Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticKind::Malformed,
        message,
    )
    .in_stream(entry.path.clone().unwrap_or_default())
    .with_detail("entry_id", &entry.id)
    .with_detail("decoded_offset", decoded_offset.to_string())
}

fn read_u16(data: &[u8], cursor: &mut usize) -> Option<u16> {
    let end = cursor.checked_add(2)?;
    let raw: [u8; 2] = data.get(*cursor..end)?.try_into().ok()?;
    *cursor = end;
    Some(u16::from_le_bytes(raw))
}

fn read_u8(data: &[u8], cursor: &mut usize) -> Option<u8> {
    let value = *data.get(*cursor)?;
    *cursor = cursor.checked_add(1)?;
    Some(value)
}

fn read_u32_le(data: &[u8], cursor: &mut usize) -> Option<u32> {
    let end = cursor.checked_add(4)?;
    let raw: [u8; 4] = data.get(*cursor..end)?.try_into().ok()?;
    *cursor = end;
    Some(u32::from_le_bytes(raw))
}

fn u32_be(data: &[u8], offset: usize) -> Option<u32> {
    let raw: [u8; 4] = data.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(raw))
}

fn sha256_hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(data);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

const fn document_kind_rank(kind: DocumentKind) -> u8 {
    match kind {
        DocumentKind::Part => 0,
        DocumentKind::Assembly => 1,
        DocumentKind::Drawing => 2,
        DocumentKind::Unknown => 3,
    }
}

const fn document_kind_name(kind: DocumentKind) -> &'static str {
    match kind {
        DocumentKind::Part => "part",
        DocumentKind::Assembly => "assembly",
        DocumentKind::Drawing => "drawing",
        DocumentKind::Unknown => "unknown",
    }
}
