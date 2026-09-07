use std::collections::{BTreeMap, BTreeSet};

use roxmltree::{Document, Node, ParsingOptions};
use sha2::{Digest, Sha256};
use sldkit_container::decode_selected_bytes;
use sldkit_core::{
    ContainerInventory, Diagnostic, DiagnosticKind, DiagnosticSeverity, DocumentKind,
    DrawingBytePartitionStatus, DrawingCarrier, DrawingCarrierRole, DrawingRecord,
    DrawingRecordClass, DrawingRecordSource, DrawingStructureCoverage, DrawingStructureDocument,
    DrawingStructureResult, DrawingStructureSheet, DrawingStructureStatus, DrawingStructureView,
    InventoryEntry, InventoryEntryState, ParseResult, ParseStatus, ResourceLimits, SourceDocument,
    SourceInputKind, ValueOrigin,
};

const KEYWORDS_PATH: &str = "swXmlContents/KeyWords";

pub(crate) fn decode(
    data: &[u8],
    filename: Option<&str>,
    input_kind: SourceInputKind,
    limits: &ResourceLimits,
) -> DrawingStructureResult {
    let parsed = super::parse_bytes_with_source(data, filename, input_kind, limits);
    let DrawingInputs {
        document,
        inventory,
        mut diagnostics,
    } = match require_drawing_inputs(parsed) {
        Ok(inputs) => inputs,
        Err(result) => return *result,
    };
    let (batch, malformed_keywords) =
        inventory_keywords(data, &inventory.entries, limits, &mut diagnostics);
    let RecordBatch {
        records,
        sheets,
        views,
    } = batch;
    let source_streams = drawing_carriers(&inventory.entries, &mut diagnostics);
    let coverage = drawing_coverage(&records, &sheets, &views, &source_streams);
    let status = drawing_status(&records, &source_streams, &coverage, malformed_keywords);
    append_structure_diagnostics(&coverage, &source_streams, &mut diagnostics);

    DrawingStructureResult {
        status,
        structure: Some(DrawingStructureDocument {
            source: document.source,
            internal_version: document.internal_version,
            records,
            sheets,
            views,
            source_streams,
            coverage,
        }),
        diagnostics,
    }
}

struct DrawingInputs {
    document: SourceDocument,
    inventory: ContainerInventory,
    diagnostics: Vec<Diagnostic>,
}

fn require_drawing_inputs(
    parsed: ParseResult,
) -> Result<DrawingInputs, Box<DrawingStructureResult>> {
    let ParseResult {
        status,
        document,
        inventory,
        mut diagnostics,
        ..
    } = parsed;
    let terminal_status = match status {
        ParseStatus::Rejected => Some(DrawingStructureStatus::Rejected),
        ParseStatus::Malformed => Some(DrawingStructureStatus::Malformed),
        ParseStatus::Unsupported => Some(DrawingStructureStatus::Unsupported),
        ParseStatus::Parsed | ParseStatus::Partial => None,
    };
    if let Some(status) = terminal_status {
        return Err(Box::new(DrawingStructureResult {
            status,
            structure: None,
            diagnostics,
        }));
    }

    let Some(document) = document else {
        diagnostics.push(Diagnostic::new(
            "drawing.structure_document_missing",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "Drawing structure inventory requires a parsed source document",
        ));
        return Err(Box::new(DrawingStructureResult {
            status: DrawingStructureStatus::Malformed,
            structure: None,
            diagnostics,
        }));
    };
    if document.document_kind.value != DocumentKind::Drawing
        || document.document_kind.origin != ValueOrigin::Source
    {
        diagnostics.push(
            Diagnostic::new(
                "drawing.structure_document_kind_unsupported",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Unsupported,
                "Drawing structure inventory requires source evidence for a modern Drawing",
            )
            .with_detail(
                "document_kind",
                format!("{:?}", document.document_kind.value).to_ascii_lowercase(),
            )
            .with_detail(
                "document_kind_origin",
                format!("{:?}", document.document_kind.origin).to_ascii_lowercase(),
            ),
        );
        return Err(Box::new(DrawingStructureResult {
            status: DrawingStructureStatus::Unsupported,
            structure: None,
            diagnostics,
        }));
    }
    let Some(inventory) = inventory else {
        diagnostics.push(Diagnostic::new(
            "drawing.structure_inventory_missing",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "Drawing structure inventory requires the validated container inventory",
        ));
        return Err(Box::new(DrawingStructureResult {
            status: DrawingStructureStatus::Malformed,
            structure: None,
            diagnostics,
        }));
    };

    Ok(DrawingInputs {
        document,
        inventory,
        diagnostics,
    })
}

fn inventory_keywords(
    data: &[u8],
    entries: &[InventoryEntry],
    limits: &ResourceLimits,
    diagnostics: &mut Vec<Diagnostic>,
) -> (RecordBatch, u64) {
    let keyword_entries = entries
        .iter()
        .filter(|entry| {
            entry.state == InventoryEntryState::Decoded
                && entry.path.as_deref() == Some(KEYWORDS_PATH)
        })
        .collect::<Vec<_>>();
    let wanted = keyword_entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    let decoded = decode_selected_bytes(data, &wanted, limits);

    let mut batch = RecordBatch::default();
    let mut malformed_keywords = 0_u64;
    for entry in keyword_entries {
        let Some(bytes) = decoded.streams.get(&entry.id) else {
            malformed_keywords = malformed_keywords.saturating_add(1);
            diagnostics.push(
                Diagnostic::new(
                    "drawing.keywords_stream_unavailable",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "a decoded Drawing keyword stream was unavailable during M6a inventory",
                )
                .in_stream(KEYWORDS_PATH)
                .with_detail("entry_id", &entry.id),
            );
            continue;
        };
        let Some(record_batch) = inventory_keyword_records(entry, bytes, limits, diagnostics)
        else {
            malformed_keywords = malformed_keywords.saturating_add(1);
            continue;
        };
        batch.records.extend(record_batch.records);
        batch.sheets.extend(record_batch.sheets);
        batch.views.extend(record_batch.views);
    }
    (batch, malformed_keywords)
}

fn drawing_status(
    records: &[DrawingRecord],
    source_streams: &[DrawingCarrier],
    coverage: &DrawingStructureCoverage,
    malformed_keywords: u64,
) -> DrawingStructureStatus {
    let has_untyped_records = records.iter().any(|record| {
        !matches!(
            record.class,
            DrawingRecordClass::Root
                | DrawingRecordClass::Sheet
                | DrawingRecordClass::View
                | DrawingRecordClass::Field
        )
    });
    let partial = coverage.partition_status != DrawingBytePartitionStatus::Complete
        || malformed_keywords > 0
        || records.is_empty()
        || !source_streams.is_empty()
        || has_untyped_records;
    if partial {
        DrawingStructureStatus::Partial
    } else {
        DrawingStructureStatus::Inventoried
    }
}

fn append_structure_diagnostics(
    coverage: &DrawingStructureCoverage,
    source_streams: &[DrawingCarrier],
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(
        Diagnostic::new(
            "drawing.structure_xml_inventory",
            DiagnosticSeverity::Info,
            DiagnosticKind::Preserved,
            "source XML Drawing records were inventoried with stable IDs and exact decoded ranges",
        )
        .with_detail("record_count", coverage.record_count.to_string())
        .with_detail("sheet_count", coverage.supported_sheet_count.to_string())
        .with_detail("sheet_view_count", coverage.sheet_view_count.to_string()),
    );
    if coverage.partition_status == DrawingBytePartitionStatus::Incomplete {
        diagnostics.push(
            Diagnostic::new(
                "drawing.byte_partition_incomplete",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Unsupported,
                "Drawing record bytes do not yet have an exclusive typed/uninterpreted partition",
            )
            .with_detail("partition_status", "incomplete"),
        );
    }
    if !source_streams.is_empty() {
        diagnostics.push(
            Diagnostic::new(
                "drawing.carrier_record_framing_unverified",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Unsupported,
                "Drawing carrier streams are retained exactly but their record framing is not verified",
            )
            .with_detail(
                "candidate_stream_count",
                coverage.candidate_stream_count.to_string(),
            )
            .with_detail(
                "candidate_stream_bytes",
                coverage.candidate_stream_bytes.to_string(),
            ),
        );
    }
    if coverage.unassigned_view_record_count > 0 {
        diagnostics.push(
            Diagnostic::new(
                "drawing.view_records_unassigned",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "View records without direct sheet membership remain in the source record inventory",
            )
            .with_detail(
                "count",
                coverage.unassigned_view_record_count.to_string(),
            ),
        );
    }
}

#[derive(Default)]
struct RecordBatch {
    records: Vec<DrawingRecord>,
    sheets: Vec<DrawingStructureSheet>,
    views: Vec<DrawingStructureView>,
}

fn inventory_keyword_records(
    entry: &InventoryEntry,
    bytes: &[u8],
    limits: &ResourceLimits,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RecordBatch> {
    let (xml_start, document) = parse_keyword_document(bytes, limits, diagnostics)?;
    let nodes = document
        .descendants()
        .filter(Node::is_element)
        .collect::<Vec<_>>();
    let ids_by_range = record_ids(entry, bytes, xml_start, &nodes);
    let records = collect_keyword_records(entry, bytes, xml_start, &nodes, &ids_by_range);
    let (sheets, views) = collect_sheet_records(&nodes, &ids_by_range);

    Some(RecordBatch {
        records,
        sheets,
        views,
    })
}

fn parse_keyword_document<'a>(
    bytes: &'a [u8],
    limits: &ResourceLimits,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(usize, Document<'a>)> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limits.max_xml_stream_bytes {
        diagnostics.push(
            Diagnostic::new(
                "limit.xml_stream_bytes",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "decoded Drawing keyword XML exceeds the configured semantic parser limit",
            )
            .in_stream(KEYWORDS_PATH)
            .with_detail("actual_bytes", bytes.len().to_string())
            .with_detail("limit_bytes", limits.max_xml_stream_bytes.to_string()),
        );
        return None;
    }
    let Some(xml_start) = bytes.iter().position(|value| *value == b'<') else {
        diagnostics.push(
            Diagnostic::new(
                "drawing.keywords_xml_root_missing",
                DiagnosticSeverity::Error,
                DiagnosticKind::Malformed,
                "Drawing keyword stream has no XML document start",
            )
            .in_stream(KEYWORDS_PATH),
        );
        return None;
    };
    let text_bytes = bytes.get(xml_start..)?;
    let Ok(text) = std::str::from_utf8(text_bytes) else {
        diagnostics.push(
            Diagnostic::new(
                "drawing.keywords_xml_encoding_invalid",
                DiagnosticSeverity::Error,
                DiagnosticKind::Malformed,
                "Drawing keyword XML is not valid UTF-8 after its document start",
            )
            .in_stream(KEYWORDS_PATH),
        );
        return None;
    };
    let options = ParsingOptions {
        allow_dtd: false,
        nodes_limit: limits.max_xml_nodes,
        entity_resolver: None,
    };
    let document = match Document::parse_with_options(text, options) {
        Ok(document) => document,
        Err(error) => {
            let kind = if matches!(error, roxmltree::Error::NodesLimitReached) {
                DiagnosticKind::Fatal
            } else {
                DiagnosticKind::Malformed
            };
            diagnostics.push(
                Diagnostic::new(
                    "drawing.keywords_xml_malformed",
                    DiagnosticSeverity::Error,
                    kind,
                    "Drawing keyword XML is malformed or exceeds its node limit",
                )
                .in_stream(KEYWORDS_PATH)
                .with_detail("error", error.to_string()),
            );
            return None;
        }
    };
    Some((xml_start, document))
}

fn record_ids(
    entry: &InventoryEntry,
    bytes: &[u8],
    xml_start: usize,
    nodes: &[Node<'_, '_>],
) -> BTreeMap<(usize, usize), String> {
    let mut ids_by_range = BTreeMap::new();
    for node in nodes {
        let range = node.range();
        let absolute_offset = xml_start.saturating_add(range.start);
        let Some(raw) = bytes.get(absolute_offset..xml_start.saturating_add(range.end)) else {
            continue;
        };
        ids_by_range.insert(
            (range.start, range.end),
            stable_record_id(&entry.id, absolute_offset, raw),
        );
    }
    ids_by_range
}

fn collect_keyword_records(
    entry: &InventoryEntry,
    bytes: &[u8],
    xml_start: usize,
    nodes: &[Node<'_, '_>],
    ids_by_range: &BTreeMap<(usize, usize), String>,
) -> Vec<DrawingRecord> {
    let mut records = Vec::new();
    for node in nodes {
        let range = node.range();
        let absolute_offset = xml_start.saturating_add(range.start);
        let absolute_end = xml_start.saturating_add(range.end);
        let Some(raw) = bytes.get(absolute_offset..absolute_end) else {
            continue;
        };
        let Some(id) = ids_by_range.get(&(range.start, range.end)).cloned() else {
            continue;
        };
        let parent_id = node
            .parent_element()
            .and_then(|parent| {
                let parent_range = parent.range();
                ids_by_range.get(&(parent_range.start, parent_range.end))
            })
            .cloned();
        let source_attributes = node
            .attributes()
            .map(|attribute| (attribute.name().to_owned(), attribute.value().to_owned()))
            .collect::<BTreeMap<_, _>>();
        records.push(DrawingRecord {
            id,
            class: record_class(*node),
            source_tag: node.tag_name().name().to_owned(),
            parent_id,
            source_id: node.attribute("id").map(str::to_owned),
            name: node.attribute("Name").map(str::to_owned),
            source_type: node.attribute("Type").map(str::to_owned),
            source_attributes,
            direct_text: direct_text(*node),
            source: DrawingRecordSource {
                entry_id: entry.id.clone(),
                stream_path: KEYWORDS_PATH.to_owned(),
                decoded_offset: u64::try_from(absolute_offset).unwrap_or(u64::MAX),
                byte_len: u64::try_from(raw.len()).unwrap_or(u64::MAX),
                sha256: sha256_hex(raw),
            },
        });
    }
    records
}

fn collect_sheet_records(
    nodes: &[Node<'_, '_>],
    ids_by_range: &BTreeMap<(usize, usize), String>,
) -> (Vec<DrawingStructureSheet>, Vec<DrawingStructureView>) {
    let mut sheets = Vec::new();
    let mut views = Vec::new();
    for sheet_node in nodes.iter().filter(|node| node.has_tag_name("Sheet")) {
        let direct_views = sheet_node
            .children()
            .filter(|node| node.has_tag_name("View"))
            .collect::<Vec<_>>();
        if sheet_node.attribute("Type") != Some("Sheet") && direct_views.is_empty() {
            continue;
        }
        let sheet_range = sheet_node.range();
        let Some(sheet_id) = ids_by_range
            .get(&(sheet_range.start, sheet_range.end))
            .cloned()
        else {
            continue;
        };
        let mut view_record_ids = Vec::new();
        for view_node in direct_views {
            let view_range = view_node.range();
            let Some(view_id) = ids_by_range
                .get(&(view_range.start, view_range.end))
                .cloned()
            else {
                continue;
            };
            view_record_ids.push(view_id.clone());
            views.push(DrawingStructureView {
                record_id: view_id,
                sheet_record_id: Some(sheet_id.clone()),
                source_id: view_node.attribute("id").map(str::to_owned),
                name: view_node.attribute("Name").map(str::to_owned),
                referenced_document: direct_text(view_node),
                referenced_configuration: view_node.attribute("Description").map(str::to_owned),
                parent_view_record_id: None,
            });
        }
        sheets.push(DrawingStructureSheet {
            record_id: sheet_id,
            source_id: sheet_node.attribute("id").map(str::to_owned),
            name: sheet_node.attribute("Name").map(str::to_owned),
            source_type: sheet_node.attribute("Type").map(str::to_owned),
            view_record_ids,
        });
    }
    (sheets, views)
}

fn drawing_carriers(
    entries: &[InventoryEntry],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<DrawingCarrier> {
    let mut carriers = Vec::new();
    for entry in entries {
        if entry.state != InventoryEntryState::Decoded {
            continue;
        }
        let Some(path) = entry.path.as_deref() else {
            continue;
        };
        let role = match path {
            "Contents/Definition" => DrawingCarrierRole::DefinitionCandidate,
            "Contents/DisplayLists" => DrawingCarrierRole::DisplayListsCandidate,
            "Contents/VBLists" => DrawingCarrierRole::VbListsCandidate,
            _ => continue,
        };
        let (Some(decoded_size), Some(decoded_sha256)) =
            (entry.decoded_size, entry.decoded_sha256.clone())
        else {
            diagnostics.push(
                Diagnostic::new(
                    "drawing.carrier_metadata_incomplete",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "Drawing carrier inventory entry lacks decoded size or digest",
                )
                .in_stream(path)
                .with_detail("entry_id", &entry.id),
            );
            continue;
        };
        carriers.push(DrawingCarrier {
            entry_id: entry.id.clone(),
            stream_path: path.to_owned(),
            role,
            decoded_size,
            decoded_sha256,
            record_framing_verified: false,
        });
    }
    carriers
}

fn drawing_coverage(
    records: &[DrawingRecord],
    sheets: &[DrawingStructureSheet],
    views: &[DrawingStructureView],
    carriers: &[DrawingCarrier],
) -> DrawingStructureCoverage {
    let mut record_class_counts = BTreeMap::new();
    for record in records {
        *record_class_counts
            .entry(record_class_name(record.class).to_owned())
            .or_insert(0_u64) += 1;
    }
    let sheet_record_count = records
        .iter()
        .filter(|record| record.class == DrawingRecordClass::Sheet)
        .count();
    let view_record_count = records
        .iter()
        .filter(|record| record.class == DrawingRecordClass::View)
        .count();
    let unique_record_range_count = records
        .iter()
        .map(|record| {
            (
                record.source.entry_id.as_str(),
                record.source.decoded_offset,
                record.source.byte_len,
            )
        })
        .collect::<BTreeSet<_>>()
        .len();
    DrawingStructureCoverage {
        record_count: saturating_u64(records.len()),
        record_class_counts,
        sheet_record_count: saturating_u64(sheet_record_count),
        supported_sheet_count: saturating_u64(sheets.len()),
        sheet_view_count: saturating_u64(views.len()),
        unassigned_view_record_count: saturating_u64(view_record_count.saturating_sub(views.len())),
        candidate_stream_count: saturating_u64(carriers.len()),
        candidate_stream_bytes: carriers.iter().fold(0_u64, |total, carrier| {
            total.saturating_add(carrier.decoded_size)
        }),
        located_record_count: saturating_u64(records.len()),
        unique_record_range_count: saturating_u64(unique_record_range_count),
        partition_status: DrawingBytePartitionStatus::Incomplete,
        typed_bytes: None,
        uninterpreted_bytes: None,
    }
}

fn record_class(node: Node<'_, '_>) -> DrawingRecordClass {
    match node.tag_name().name() {
        "Keywords" | "root" => DrawingRecordClass::Root,
        "Attribute" => DrawingRecordClass::Attribute,
        "Feature" => DrawingRecordClass::Feature,
        "Layer" => DrawingRecordClass::Layer,
        "Note" => DrawingRecordClass::Note,
        "Reference" => DrawingRecordClass::Reference,
        "Sheet" => DrawingRecordClass::Sheet,
        "View" => DrawingRecordClass::View,
        "Sketch" => DrawingRecordClass::Sketch,
        _ if node
            .parent_element()
            .is_some_and(|parent| parent.has_tag_name("Sheet")) =>
        {
            DrawingRecordClass::Field
        }
        _ => DrawingRecordClass::Other,
    }
}

const fn record_class_name(class: DrawingRecordClass) -> &'static str {
    match class {
        DrawingRecordClass::Root => "root",
        DrawingRecordClass::Attribute => "attribute",
        DrawingRecordClass::Feature => "feature",
        DrawingRecordClass::Layer => "layer",
        DrawingRecordClass::Note => "note",
        DrawingRecordClass::Reference => "reference",
        DrawingRecordClass::Sheet => "sheet",
        DrawingRecordClass::View => "view",
        DrawingRecordClass::Sketch => "sketch",
        DrawingRecordClass::Field => "field",
        DrawingRecordClass::Other => "other",
    }
}

fn direct_text(node: Node<'_, '_>) -> Option<String> {
    let mut value = String::new();
    for child in node.children().filter(Node::is_text) {
        if let Some(text) = child.text() {
            value.push_str(text);
        }
    }
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn stable_record_id(entry_id: &str, decoded_offset: usize, raw: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(entry_id.as_bytes());
    hasher.update([0]);
    hasher.update(
        u64::try_from(decoded_offset)
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(u64::try_from(raw.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(Sha256::digest(raw));
    format!("drawing:record:{}", digest_hex(hasher.finalize()))
}

fn sha256_hex(data: &[u8]) -> String {
    digest_hex(Sha256::digest(data))
}

fn digest_hex(digest: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = digest.as_ref();
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
