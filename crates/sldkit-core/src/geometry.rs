use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Diagnostic, SourceInfo};

/// Outcome of the explicit modern-Part geometry capability.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryStatus {
    Decoded,
    Partial,
    Unsupported,
    Malformed,
    Rejected,
}

/// Semantic role of one exact container entry considered by geometry decode.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryStreamRole {
    ParasolidPartition,
    ParasolidDeltas,
    Tessellation,
}

/// Why one candidate participates in the decoded geometry state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryStreamSelection {
    Active,
    Alternate,
    Supporting,
    Candidate,
}

/// Exact outer-container entry backing geometry or tessellation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryStreamCandidate {
    pub entry_id: String,
    pub stream_path: String,
    pub role: GeometryStreamRole,
    pub selection: GeometryStreamSelection,
    pub decoded_size: Option<u64>,
    pub decoded_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selection_evidence: Vec<String>,
}

/// Relationship between a decoded value and the source bytes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryExactness {
    ByteExact,
    Derived,
    Inferred,
    Unknown,
}

/// Whether geometry-domain bytes have an exclusive typed/uninterpreted range map.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryBytePartitionStatus {
    Complete,
    #[default]
    Incomplete,
}

/// Sparse source location and exactness for one topology or geometry entity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryEntityProvenance {
    pub stream: Option<String>,
    pub offset: Option<u64>,
    pub tag: Option<String>,
    pub exactness: GeometryExactness,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_exactness: BTreeMap<String, GeometryExactness>,
}

/// Native object identity and effective display state attached to a carrier.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometrySourceObject {
    pub format: String,
    pub object_id: String,
    pub name: Option<String>,
    pub color: Option<[f32; 4]>,
    pub visible: Option<bool>,
    pub layer: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instance_path: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryBody {
    pub id: String,
    pub kind: String,
    pub region_ids: Vec<String>,
    pub transform: Option<Value>,
    pub name: Option<String>,
    pub color: Option<[f32; 4]>,
    pub visible: Option<bool>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryRegion {
    pub id: String,
    pub body_id: String,
    pub shell_ids: Vec<String>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryShell {
    pub id: String,
    pub region_id: String,
    pub face_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wire_edge_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub free_vertex_ids: Vec<String>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryFace {
    pub id: String,
    pub shell_id: String,
    pub surface_id: String,
    pub sense: String,
    pub loop_ids: Vec<String>,
    pub name: Option<String>,
    pub color: Option<[f32; 4]>,
    pub tolerance: Option<f64>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryPcurveUse {
    pub pcurve_id: String,
    pub isoparametric: Option<bool>,
    pub parameter_range: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryVertexUse {
    pub vertex_id: String,
    pub after_coedge_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pcurves: Vec<GeometryPcurveUse>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryLoop {
    pub id: String,
    pub face_id: String,
    pub boundary_role: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coedge_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vertex_uses: Vec<GeometryVertexUse>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryCoedge {
    pub id: String,
    pub loop_id: String,
    pub edge_id: String,
    pub next_id: String,
    pub previous_id: String,
    pub radial_next_id: String,
    pub sense: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pcurves: Vec<GeometryPcurveUse>,
    pub use_curve_id: Option<String>,
    pub use_curve_parameter_range: Option<[f64; 2]>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryEdge {
    pub id: String,
    pub curve_id: Option<String>,
    pub start_vertex_id: String,
    pub end_vertex_id: String,
    pub parameter_range: Option<[f64; 2]>,
    pub tolerance: Option<f64>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryVertex {
    pub id: String,
    pub point_id: String,
    pub tolerance: Option<f64>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryPoint {
    pub id: String,
    pub position: [f64; 3],
    pub source_object: Option<GeometrySourceObject>,
    pub provenance: GeometryEntityProvenance,
}

/// Typed carrier family with its lossless serialized parameters.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryCarrierDomain {
    Surface,
    Curve,
    Pcurve,
}

/// Source fields that qualify a parameter-space carrier beyond its shape.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryPcurveState {
    pub wrapper_reversed: Option<bool>,
    pub native_tail_flags: Option<[bool; 4]>,
    pub parameter_range: Option<[f64; 2]>,
    pub fit_tolerance: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryCarrier {
    pub id: String,
    pub domain: GeometryCarrierDomain,
    pub kind: String,
    pub definition: Value,
    pub raw_record_id: Option<String>,
    pub source_object: Option<GeometrySourceObject>,
    pub pcurve_state: Option<GeometryPcurveState>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryConstructionDomain {
    Surface,
    Curve,
}

/// Procedural construction referenced by an exact solved carrier.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryConstruction {
    pub id: String,
    pub domain: GeometryConstructionDomain,
    pub produced_carrier_id: String,
    pub definition: Value,
    pub cache_fit_tolerance: Option<f64>,
    pub record_bounds: Option<[Option<f64>; 4]>,
    pub raw_record_id: Option<String>,
    pub provenance: GeometryEntityProvenance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryTessellationChannel {
    pub domain: String,
    pub item_size: u32,
    pub kind: u32,
    pub flags: u32,
    pub count: u32,
    pub byte_len: u64,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryTessellationTriangleGroup {
    pub source_id: Option<String>,
    pub triangles: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryTessellationTextureAssignment {
    pub source_id: Option<String>,
    pub texture_id: String,
    pub triangles: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryTessellation {
    pub id: String,
    pub body_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub face_ids: Vec<String>,
    pub chordal_deflection: Option<f64>,
    pub source_object: Option<GeometrySourceObject>,
    pub vertices: Vec<[f64; 3]>,
    pub triangles: Vec<[u32; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feature_edges: Vec<[u32; 2]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strip_lengths: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub normals: Vec<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corner_normals: Vec<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triangle_groups: Vec<GeometryTessellationTriangleGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub texture_assignments: Vec<GeometryTessellationTextureAssignment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<GeometryTessellationChannel>,
    pub provenance: GeometryEntityProvenance,
}

/// Configuration-specific body membership; `None` means unresolved, not empty.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryConfigurationState {
    pub id: String,
    pub ordinal: u32,
    pub active: Option<bool>,
    pub source_index: Option<u32>,
    pub name: Option<String>,
    pub body_ids: Option<Vec<String>>,
}

/// Per-body topology census and an applicable cell-complex Euler value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryTopologyMetrics {
    pub body_id: String,
    pub regions: u64,
    pub shells: u64,
    pub faces: u64,
    pub loops: u64,
    pub coedges: u64,
    pub edges: u64,
    pub vertices: u64,
    pub euler_characteristic: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct GeometryModel {
    #[serde(default)]
    pub bodies: Vec<GeometryBody>,
    #[serde(default)]
    pub regions: Vec<GeometryRegion>,
    #[serde(default)]
    pub shells: Vec<GeometryShell>,
    #[serde(default)]
    pub faces: Vec<GeometryFace>,
    #[serde(default)]
    pub loops: Vec<GeometryLoop>,
    #[serde(default)]
    pub coedges: Vec<GeometryCoedge>,
    #[serde(default)]
    pub edges: Vec<GeometryEdge>,
    #[serde(default)]
    pub vertices: Vec<GeometryVertex>,
    #[serde(default)]
    pub points: Vec<GeometryPoint>,
    #[serde(default)]
    pub carriers: Vec<GeometryCarrier>,
    #[serde(default)]
    pub constructions: Vec<GeometryConstruction>,
    #[serde(default)]
    pub tessellations: Vec<GeometryTessellation>,
}

/// Retained native bytes remain available from the exact input image.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryRawRecord {
    pub id: String,
    pub stream: String,
    pub offset: u64,
    pub byte_len: u64,
    pub sha256: String,
    pub data_retained: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryLoss {
    pub code: String,
    pub taxonomy: String,
    pub category: String,
    pub severity: String,
    pub message: String,
    pub stream: Option<String>,
    pub offset: Option<u64>,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryFinding {
    pub check: String,
    pub severity: String,
    pub message: String,
    pub entity_id: Option<String>,
}

/// Storage used for a nested Parasolid stream inside its outer container entry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryByteStorage {
    Direct,
    WrappedZlib,
}

/// Coordinate system used by entity offsets and future classified ranges.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryByteOffsetBasis {
    ParasolidBody,
}

/// Exact nested byte space to which geometry provenance offsets may refer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryByteDomain {
    pub id: String,
    pub container_entry_id: String,
    pub stream_path: String,
    pub role: GeometryStreamRole,
    pub storage: GeometryByteStorage,
    /// Start of the direct stream or zlib member in the decoded outer payload.
    pub outer_payload_offset: u64,
    pub description: String,
    pub schema: String,
    /// Size and digest of the complete inflated `PS` stream, including header.
    pub stream_byte_len: u64,
    pub stream_sha256: String,
    /// Start of the body relative to the complete inflated `PS` stream.
    pub body_offset: u64,
    /// Size and digest of the exact body-relative byte domain.
    pub byte_len: u64,
    pub sha256: String,
    pub offset_basis: GeometryByteOffsetBasis,
}

/// Byte counts intentionally separate exact extraction from semantic transfer.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryByteCoverage {
    pub source_bytes: u64,
    pub candidate_stream_bytes: u64,
    /// Decoded bytes in active outer-container entries.
    pub active_stream_bytes: u64,
    /// Bytes in the nested geometry domains selected for range partitioning.
    pub partition_domain_bytes: u64,
    pub retained_record_bytes: u64,
    /// Entities with a source location in an active geometry stream. A location
    /// is an anchor, not a byte range.
    pub located_entity_count: u64,
    /// Distinct `(stream, offset)` anchors among located entities.
    pub unique_location_count: u64,
    /// Geometry-domain bytes covered by non-overlapping classified ranges.
    /// The name is retained for compatibility with the initial API.
    pub classified_active_bytes: u64,
    /// Geometry-domain bytes for which no safe range classification is available.
    /// The name is retained for compatibility with the initial API.
    pub unclassified_active_bytes: u64,
    pub partition_status: GeometryBytePartitionStatus,
    /// `None` means typed record lengths are not available.
    pub typed_bytes: Option<u64>,
    /// `None` means the decoder cannot partition every geometry-domain byte into
    /// typed versus uninterpreted ranges.
    pub uninterpreted_bytes: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryFidelityReport {
    pub decoder: String,
    pub decoder_version: String,
    pub geometry_transferred: bool,
    #[serde(default)]
    pub entity_counts: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub byte_domains: Vec<GeometryByteDomain>,
    pub byte_coverage: GeometryByteCoverage,
    #[serde(default)]
    pub losses: Vec<GeometryLoss>,
    #[serde(default)]
    pub validation_findings: Vec<GeometryFinding>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryDocument {
    pub source: SourceInfo,
    pub length_unit: String,
    #[serde(default)]
    pub source_streams: Vec<GeometryStreamCandidate>,
    pub model: GeometryModel,
    #[serde(default)]
    pub configurations: Vec<GeometryConfigurationState>,
    #[serde(default)]
    pub topology_metrics: Vec<GeometryTopologyMetrics>,
    #[serde(default)]
    pub raw_records: Vec<GeometryRawRecord>,
    pub fidelity: GeometryFidelityReport,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryResult {
    pub status: GeometryStatus,
    pub geometry: Option<GeometryDocument>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}
