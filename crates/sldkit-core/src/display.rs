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
    /// Index into [`DisplayMeshDocument::instances`] of the occurrence that
    /// owns this table. `FaceTessellations` sections only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_index: Option<u32>,
}

/// One component occurrence listed by an assembly's `FaceTessellations/Directory`.
///
/// SOLIDWORKS 2023 and later assemblies save the display tables of each
/// referenced part once, in a section of a `FaceTessellations/<stream>`
/// entry, and list every occurrence in the directory. An occurrence that
/// repeats a part names the owner instead of a section. The tables stay in
/// the referenced part's coordinates; placement is the caller's join with the
/// component transforms.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DisplayMeshInstance {
    /// Saved occurrence path, root first: `<name>-<n>@<assembly>`, or
    /// `<sub>-<m>@<assembly>/<name>-<n>@<sub>` for a nested occurrence.
    pub path: String,
    /// `<name>` of the last path segment.
    pub name: String,
    /// `<n>` of the last path segment.
    pub number: Option<u32>,
    /// Position in the saved component tree. Sub-assembly nodes consume an
    /// index without a record of their own.
    pub tree_index: u32,
    /// Index of the instance whose tables this occurrence displays: itself
    /// when it owns a section, the owner when it repeats one, none when the
    /// file saved no tables for it.
    pub tables_from: Option<u32>,
    /// Stream (index into [`DisplayMeshDocument::streams`]) and section
    /// ordinal of the owned tables.
    pub stream_index: Option<u32>,
    pub section_index: Option<u32>,
    /// Face count the section declares and the number of validated tables.
    pub declared_face_count: Option<u32>,
    pub face_count: u32,
    /// Bodies the directory lists for the occurrence.
    pub body_count: u32,
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
    /// Component occurrences of a `FaceTessellations` assembly; empty otherwise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instances: Vec<DisplayMeshInstance>,
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
