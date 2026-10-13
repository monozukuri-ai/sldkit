use serde::{Deserialize, Serialize};

use crate::{Diagnostic, GeometryLoss, SourceInfo};

/// Outcome of the kernel-free evaluation of a decoded Part geometry document.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryEvaluationStatus {
    /// Every edge was sampled and every edge smoothness was established.
    Evaluated,
    /// Some edges, normals, bodies, or meshes were withheld; see `losses`.
    Partial,
    /// The input provided no geometry document to evaluate.
    Unavailable,
    /// The options were invalid or named a configuration that does not exist.
    Rejected,
}

/// Caller-controlled sampling and classification thresholds.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct GeometryEvaluationOptions {
    /// Maximum chord deviation of a sampled polyline from its exact curve, in mm.
    pub chord_tolerance_mm: f64,
    /// Upper bound on polyline segments produced for one edge.
    pub max_segments_per_edge: u32,
    /// Dihedral angles at or below this value classify an edge as tangent.
    pub tangent_angle_deg: f64,
    /// Mesh triangle pairs meeting at or above this angle produce feature edges.
    pub mesh_feature_angle_deg: f64,
    /// Restrict edges, meshes, and the model bounding box to this configuration.
    pub configuration: Option<String>,
}

impl Default for GeometryEvaluationOptions {
    fn default() -> Self {
        Self {
            chord_tolerance_mm: 0.01,
            max_segments_per_edge: 512,
            tangent_angle_deg: 0.5,
            mesh_feature_angle_deg: 30.0,
            configuration: None,
        }
    }
}

/// Where an edge's parameter interval came from.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeIntervalSource {
    /// The decoder's own `parameter_range`.
    Source,
    /// The adapter's endpoint-derived interval.
    Derived,
    /// No usable interval; the edge cannot be sampled.
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeSampleStatus {
    Sampled,
    Unsampled,
}

/// Classification of an edge by the outward normals of its adjacent faces.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeSmoothness {
    /// Two face uses whose outward normals differ by more than the tangent angle.
    Sharp,
    /// Two face uses whose outward normals agree within the tangent angle.
    Tangent,
    /// Exactly one face use, such as the free edge of a sheet body.
    Boundary,
    /// Normals could not be established, or the edge has no or more than two face uses.
    Unknown,
}

/// Which samples contributed to a bounding box.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundingBoxMethod {
    EdgeSamples,
    Tessellation,
    EdgeSamplesAndTessellation,
}

/// Axis-aligned extent in millimetres.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvaluatedBounds {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

/// Bounding box with an explicit method and exactness claim.
///
/// `exact` is `true` only when every contributing face is planar, every
/// contributing edge is a line, circle, or ellipse sampled over a known
/// interval with its analytic axis extrema included, no body carries an
/// unapplied transform, and no contributing edge was withheld. Tessellation
/// vertices never widen an exact box because they lie on the exact surfaces.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvaluatedBoundingBox {
    pub bounds: EvaluatedBounds,
    pub method: BoundingBoxMethod,
    pub exact: bool,
    pub edge_count: u64,
    pub vertex_count: u64,
}

/// One evaluated B-Rep edge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvaluatedEdge {
    pub edge_id: String,
    pub curve_id: Option<String>,
    pub curve_kind: String,
    pub body_id: Option<String>,
    pub interval_source: EdgeIntervalSource,
    /// Directed from the edge's start vertex to its end vertex.
    pub parameter_range: Option<[f64; 2]>,
    pub status: EdgeSampleStatus,
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub polyline: Vec<[f64; 3]>,
    pub segment_count: u32,
    /// Bound on the chord deviation actually achieved by `polyline`.
    pub chord_tolerance_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adjacent_face_ids: Vec<String>,
    pub smoothness: EdgeSmoothness,
    /// Unsigned angle between the two outward face normals at the parameter midpoint.
    pub dihedral_angle_deg: Option<f64>,
    /// The adapter synthesized this edge to close a periodic face; it is not a source edge.
    pub is_derived_seam: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvaluatedBodyBounds {
    pub body_id: String,
    pub bounding_box: Option<EvaluatedBoundingBox>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvaluatedConfigurationBounds {
    pub configuration_id: String,
    pub name: Option<String>,
    /// `None` keeps the decoder's unresolved membership; the box is then absent.
    pub body_ids: Option<Vec<String>>,
    pub bounding_box: Option<EvaluatedBoundingBox>,
}

/// Mesh-derived quantities for one display tessellation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EvaluatedMesh {
    pub tessellation_id: String,
    pub body_id: Option<String>,
    pub vertex_count: u64,
    pub triangle_count: u64,
    pub bounds: Option<EvaluatedBounds>,
    pub area_mm2: f64,
    /// `true` when every edge is shared by exactly two consistently oriented
    /// triangles; `None` when the mesh has no valid triangles.
    pub is_closed: Option<bool>,
    /// Divergence-theorem volume, positive for outward counter-clockwise winding;
    /// only present for closed meshes.
    pub signed_volume_mm3: Option<f64>,
    /// Ascending vertex-index pairs shared by two triangles meeting at or above the feature angle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub feature_edges: Vec<[u32; 2]>,
    /// Ascending vertex-index pairs used by exactly one triangle.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub boundary_edges: Vec<[u32; 2]>,
    pub feature_angle_deg: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct GeometryEvaluationSummary {
    pub edge_count: u64,
    pub sampled_edge_count: u64,
    pub unsampled_edge_count: u64,
    pub sharp_edge_count: u64,
    pub tangent_edge_count: u64,
    pub boundary_edge_count: u64,
    pub unknown_smoothness_count: u64,
    pub mesh_count: u64,
    pub closed_mesh_count: u64,
}

/// Evaluated view of one decoded geometry document. Values are derived from the
/// public geometry model; they never replace source carriers or trim fields.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryEvaluationDocument {
    pub source: SourceInfo,
    pub length_unit: String,
    pub options: GeometryEvaluationOptions,
    pub bounding_box: Option<EvaluatedBoundingBox>,
    #[serde(default)]
    pub body_bounds: Vec<EvaluatedBodyBounds>,
    #[serde(default)]
    pub configuration_bounds: Vec<EvaluatedConfigurationBounds>,
    #[serde(default)]
    pub edges: Vec<EvaluatedEdge>,
    #[serde(default)]
    pub meshes: Vec<EvaluatedMesh>,
    pub summary: GeometryEvaluationSummary,
    #[serde(default)]
    pub losses: Vec<GeometryLoss>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GeometryEvaluationResult {
    pub status: GeometryEvaluationStatus,
    pub evaluation: Option<GeometryEvaluationDocument>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

#[cfg(test)]
mod tests {
    use super::{GeometryEvaluationOptions, GeometryEvaluationStatus};

    #[test]
    fn options_default_and_round_trip_with_partial_json() -> Result<(), serde_json::Error> {
        let options: GeometryEvaluationOptions = serde_json::from_str("{}")?;
        assert_eq!(options, GeometryEvaluationOptions::default());
        let json = serde_json::to_string(&options)?;
        assert!(json.contains("\"chord_tolerance_mm\":0.01"));
        assert!(json.contains("\"configuration\":null"));
        let status: GeometryEvaluationStatus = serde_json::from_str("\"partial\"")?;
        assert_eq!(status, GeometryEvaluationStatus::Partial);
        Ok(())
    }
}
