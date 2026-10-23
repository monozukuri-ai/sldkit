//! Saved display meshes (`DisplayLists`) decoded without B-Rep reconstruction.
//!
//! The JSON contract describes streams and per-table parts. The packed vertex,
//! normal, index, and edge arrays travel separately in [`DisplayMeshBuffers`]
//! so large meshes are not expanded into JSON or Python objects.

use serde::{Deserialize, Serialize};

use crate::{Diagnostic, DocumentKind, Envelope, SourceInfo};

/// Outcome of the display-mesh capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayMeshStatus {
    /// Every non-empty display stream yielded validated face tables.
    Decoded,
    /// Some meshes were recovered, but a stream, limit, or selection remains unresolved.
    Partial,
    /// The document has no readable saved display data.
    Unsupported,
    Malformed,
    Rejected,
}

/// Which saved appearance level supplied a part color.
///
/// `DisplayLists` precedence is body, then feature, then face.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisplayColorSource {
    Body,
    Feature,
    Face,
}

/// One framed persistent-surface reference saved after a face table.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisplaySurfaceReference {
    /// Complete source text, kept for duplicate and conflict checks.
    pub identity: String,
    pub feature_source_id: u32,
    pub local_surface_id: u32,
}

/// One validated face tessellation table and its slices of the packed buffers.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplayMeshPart {
    pub id: String,
    /// Index into [`DisplayMeshDocument::streams`].
    pub stream_index: u32,
    /// Table order inside its stream.
    pub table_index: u32,
    /// Decoded-stream offset of the first descriptor.
    pub table_offset: u64,
    pub vertex_start: u64,
    pub vertex_count: u64,
    pub triangle_start: u64,
    pub triangle_count: u64,
    pub edge_start: u64,
    pub edge_count: u64,
    pub strip_count: u64,
    /// The table stored no normals; area-weighted vertex normals were derived.
    pub normals_derived: bool,
    pub color: Option<[f32; 4]>,
    pub color_source: Option<DisplayColorSource>,
    pub appearance_name: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_references: Vec<DisplaySurfaceReference>,
}

/// One decoded `DisplayLists` container entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisplayMeshStream {
    pub entry_id: String,
    pub stream_path: String,
    pub decoded_size: u64,
    pub decoded_sha256: Option<String>,
    pub part_count: u64,
    /// Bounded UTF-16 names after the final table; candidates, not configuration proof.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trailing_names: Vec<String>,
}

/// Axis-aligned bounds of the packed positions, in millimetres.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplayMeshBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// Saved display meshes of one document.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplayMeshDocument {
    pub source: SourceInfo,
    pub envelope: Envelope,
    pub document_kind: DocumentKind,
    pub internal_version: Option<u64>,
    /// Unit of the packed positions. Source metres are scaled once.
    pub length_unit: String,
    pub streams: Vec<DisplayMeshStream>,
    pub parts: Vec<DisplayMeshPart>,
    pub vertex_count: u64,
    pub triangle_count: u64,
    pub edge_count: u64,
    pub bounds: Option<DisplayMeshBounds>,
}

/// Display-mesh capability result. Buffers are returned beside it.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DisplayMeshResult {
    pub status: DisplayMeshStatus,
    pub mesh: Option<DisplayMeshDocument>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

/// Packed arrays for every part of a [`DisplayMeshResult`], in part order.
///
/// `positions` and `normals` hold three `f32` values per vertex. `indices`
/// holds three vertex indices per triangle and `edges` two per segment. All
/// indices address the shared vertex arrays.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayMeshBuffers {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub indices: Vec<u32>,
    pub edges: Vec<u32>,
}
