use serde::{Deserialize, Serialize};

use crate::{ContainerInventory, Diagnostic, Envelope, SourceDocument};

/// Byte accounting accompanies every probe and parse result.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct CoverageReport {
    pub total_bytes: u64,
    pub inspected_bytes: u64,
    pub decoded_bytes: u64,
    pub uninterpreted_bytes: u64,
    pub streams_total: u64,
    pub streams_decoded: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeStatus {
    Recognized,
    Unrecognized,
    Malformed,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeConfidence {
    High,
    Medium,
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProbeEvidence {
    pub code: String,
    pub offset: u64,
    pub length: u64,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProbeResult {
    pub status: ProbeStatus,
    pub envelope: Envelope,
    pub confidence: ProbeConfidence,
    #[serde(default)]
    pub evidence: Vec<ProbeEvidence>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub coverage: CoverageReport,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParseStatus {
    Parsed,
    Partial,
    Unsupported,
    Malformed,
    Rejected,
}

/// Stream-level accounting for the active semantic parser profile.
///
/// Categories are mutually exclusive and cover every decoded container stream.
/// A partially interpreted XML stream may yield supported facts while its full
/// bytes remain preserved through an [`crate::UnknownRecord`].
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticCoverage {
    pub decoded_streams_total: u64,
    pub fully_interpreted_streams: u64,
    pub partially_interpreted_streams: u64,
    pub uninterpreted_streams: u64,
    pub malformed_streams: u64,
    pub decoded_bytes_total: u64,
    pub fully_interpreted_bytes: u64,
    pub partially_interpreted_bytes: u64,
    pub uninterpreted_bytes: u64,
    pub malformed_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ParseResult {
    pub status: ParseStatus,
    pub document: Option<SourceDocument>,
    pub inventory: Option<ContainerInventory>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub coverage: CoverageReport,
    pub semantic_coverage: Option<SemanticCoverage>,
}
