use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{Diagnostic, SourceInfo, SourceValue};

/// Outcome of the explicit modern-Drawing structure inventory capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawingStructureStatus {
    Inventoried,
    Partial,
    Unsupported,
    Malformed,
    Rejected,
}

/// Source XML class of one Drawing record.
///
/// These values classify XML local element names only. They do not imply that
/// Drawing geometry, dimensions, annotations, or view transforms were decoded.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawingRecordClass {
    Root,
    Attribute,
    Feature,
    Layer,
    Note,
    Reference,
    Sheet,
    View,
    Sketch,
    Field,
    Other,
}

/// Path-derived hint for an exact stream that may carry Drawing records.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawingCarrierRole {
    DefinitionCandidate,
    DisplayListsCandidate,
    VbListsCandidate,
}

/// Whether candidate-stream bytes have an exclusive typed/uninterpreted map.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DrawingBytePartitionStatus {
    Complete,
    #[default]
    Incomplete,
}

/// Exact decoded-stream range backing one source XML record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingRecordSource {
    pub entry_id: String,
    pub stream_path: String,
    pub decoded_offset: u64,
    pub byte_len: u64,
    pub sha256: String,
}

/// One deterministic, source-native XML record from a Drawing carrier.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingRecord {
    /// Stable for identical source bytes and container entry identity.
    pub id: String,
    pub class: DrawingRecordClass,
    /// XML local element name. `class` is only a normalized index over this value.
    pub source_tag: String,
    pub parent_id: Option<String>,
    pub source_id: Option<String>,
    pub name: Option<String>,
    pub source_type: Option<String>,
    /// Parsed source name/value pairs; ordering and quoting remain in the exact source range.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub source_attributes: BTreeMap<String, String>,
    /// Trimmed text from direct text-node children only.
    pub direct_text: Option<String>,
    pub source: DrawingRecordSource,
}

/// Sheet membership that is supported by direct source XML evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingStructureSheet {
    pub record_id: String,
    pub source_id: Option<String>,
    pub name: Option<String>,
    pub source_type: Option<String>,
    #[serde(default)]
    pub view_record_ids: Vec<String>,
}

/// Drawing view identity and reference fields already exposed by source XML.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingStructureView {
    pub record_id: String,
    pub sheet_record_id: Option<String>,
    pub source_id: Option<String>,
    pub name: Option<String>,
    pub referenced_document: Option<String>,
    pub referenced_configuration: Option<String>,
    /// Reserved for a dependency proven by a controlled fixture and API capture.
    pub parent_view_record_id: Option<String>,
}

/// Exact outer-container entry retained as an unframed Drawing carrier candidate.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingCarrier {
    pub entry_id: String,
    pub stream_path: String,
    pub role: DrawingCarrierRole,
    pub decoded_size: u64,
    pub decoded_sha256: String,
    /// `false` until controlled differentials and API capture establish record spans.
    pub record_framing_verified: bool,
}

/// Machine-readable M6a coverage without inferring unavailable record lengths.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingStructureCoverage {
    pub record_count: u64,
    #[serde(default)]
    pub record_class_counts: BTreeMap<String, u64>,
    pub sheet_record_count: u64,
    pub supported_sheet_count: u64,
    pub sheet_view_count: u64,
    pub unassigned_view_record_count: u64,
    pub candidate_stream_count: u64,
    pub candidate_stream_bytes: u64,
    pub located_record_count: u64,
    pub unique_record_range_count: u64,
    pub partition_status: DrawingBytePartitionStatus,
    /// `None` until candidate streams have a verified, exclusive record partition.
    pub typed_bytes: Option<u64>,
    /// `None` until candidate streams have a verified, exclusive record partition.
    pub uninterpreted_bytes: Option<u64>,
}

/// Source-oriented M6a inventory. This type intentionally has no renderable geometry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingStructureDocument {
    pub source: SourceInfo,
    pub internal_version: Option<SourceValue<u64>>,
    #[serde(default)]
    pub records: Vec<DrawingRecord>,
    #[serde(default)]
    pub sheets: Vec<DrawingStructureSheet>,
    #[serde(default)]
    pub views: Vec<DrawingStructureView>,
    #[serde(default)]
    pub source_streams: Vec<DrawingCarrier>,
    pub coverage: DrawingStructureCoverage,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrawingStructureResult {
    pub status: DrawingStructureStatus,
    pub structure: Option<DrawingStructureDocument>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}
