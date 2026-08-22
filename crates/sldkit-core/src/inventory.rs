use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CoverageReport, Diagnostic, Envelope};

/// A half-open byte range in the original input.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ByteRange {
    pub offset: u64,
    pub length: u64,
}

impl ByteRange {
    #[must_use]
    pub const fn new(offset: u64, length: u64) -> Self {
        Self { offset, length }
    }
}

/// Result of walking an outer container without interpreting document semantics.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryStatus {
    Complete,
    Partial,
    Unsupported,
    Malformed,
    Rejected,
}

/// Structural role of one inventory item.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryEntryKind {
    Stream,
    Storage,
    Block,
    CacheCell,
    DirectoryEntry,
    ZipEntry,
}

/// How far the container layer could process an item.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryEntryState {
    Decoded,
    Stored,
    MetadataOnly,
    Unsupported,
    Malformed,
}

/// Compression framing observed by the container layer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionMethod {
    None,
    DeflateRaw,
    Zlib,
    ZipStored,
    ZipDeflate,
    Unsupported,
}

/// Checksum validation state. `NotChecked` is distinct from no checksum field.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChecksumStatus {
    Verified,
    Mismatch,
    NotPresent,
    NotChecked,
}

/// Deterministic metadata for one container item.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryEntry {
    /// Stable within identical source bytes. Callers should not infer semantics from it.
    pub id: String,
    pub path: Option<String>,
    pub kind: InventoryEntryKind,
    pub state: InventoryEntryState,
    /// Contiguous frame range when the container exposes one. Fragmented CFB streams use `None`.
    pub source_range: Option<ByteRange>,
    /// Contiguous stored payload range when it is directly addressable in the source.
    pub payload_range: Option<ByteRange>,
    pub stored_size: u64,
    pub decoded_size: Option<u64>,
    pub compression: CompressionMethod,
    pub checksum: ChecksumStatus,
    pub expected_crc32: Option<u32>,
    pub stored_sha256: Option<String>,
    pub decoded_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

/// Container facts shared by Rust, JSON, and Python without document interpretation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContainerInventory {
    pub envelope: Envelope,
    pub format_version: Option<u64>,
    #[serde(default)]
    pub entries: Vec<InventoryEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InventoryResult {
    pub status: InventoryStatus,
    pub inventory: Option<ContainerInventory>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    pub coverage: CoverageReport,
    #[serde(default)]
    pub uninterpreted_ranges: Vec<ByteRange>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionMode {
    Stored,
    Decoded,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtractionStatus {
    Extracted,
    NotFound,
    Unavailable,
    Malformed,
    Rejected,
}

/// Serializable metadata for extraction; the payload is returned out of band.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ExtractionResult {
    pub status: ExtractionStatus,
    pub mode: ExtractionMode,
    pub entry: Option<InventoryEntry>,
    pub byte_len: Option<u64>,
    pub sha256: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

/// Rust extraction result. JSON and Python bindings expose `result` plus binary bytes separately.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StreamExtraction {
    pub result: ExtractionResult,
    pub data: Option<Vec<u8>>,
}
