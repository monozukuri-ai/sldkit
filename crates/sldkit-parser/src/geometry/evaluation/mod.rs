//! Kernel-free evaluation of a decoded Part geometry document.
//!
//! The evaluation samples B-Rep edges, classifies them by adjacent face
//! normals, computes bounding boxes, and derives mesh fallbacks. Every value is
//! a derivation from the public model; carriers, trim fields, and tessellations
//! are never rewritten, and anything that cannot be established is reported as
//! unavailable instead of being approximated silently.
mod carriers;
mod mesh;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use cadmpeg_ir::{
    eval,
    geometry::{CurveGeometry, SurfaceGeometry},
    math::{Point3, Vector3},
};
use sldkit_core::{
    BoundingBoxMethod, Diagnostic, DiagnosticKind, DiagnosticSeverity, EdgeIntervalSource,
    EdgeSampleStatus, EdgeSmoothness, EvaluatedBodyBounds, EvaluatedBoundingBox, EvaluatedBounds,
    EvaluatedConfigurationBounds, EvaluatedEdge, EvaluatedMesh, GeometryCarrier, GeometryDocument,
    GeometryEdge, GeometryEvaluationDocument, GeometryEvaluationOptions, GeometryEvaluationResult,
    GeometryEvaluationStatus, GeometryEvaluationSummary, GeometryFace, GeometryLoss, GeometryModel,
    GeometryResult,
};

use carriers::SURFACE_FIT_TOLERANCE_MM;

const DERIVED_SEAM_TAG: &str = "derived_periodic_seam";
const LOSS_TAXONOMY: &str = "sldkit/evaluation";

/// Evaluate a decoded geometry result, carrying the decoder's diagnostics.
pub(crate) fn evaluate_decoded(
    decoded: GeometryResult,
    options: &GeometryEvaluationOptions,
) -> GeometryEvaluationResult {
    let GeometryResult {
        geometry,
        mut diagnostics,
        ..
    } = decoded;
    let Some(document) = geometry else {
        diagnostics.push(Diagnostic::new(
            "geometry.evaluation_unavailable",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unsupported,
            "no decoded geometry document was available to evaluate",
        ));
        return GeometryEvaluationResult {
            status: GeometryEvaluationStatus::Unavailable,
            evaluation: None,
            diagnostics,
        };
    };
    let mut result = evaluate(&document, options);
    diagnostics.append(&mut result.diagnostics);
    result.diagnostics = diagnostics;
    result
}

/// Evaluate one geometry document under the caller's options.
#[allow(clippy::too_many_lines)]
pub(crate) fn evaluate(
    document: &GeometryDocument,
    options: &GeometryEvaluationOptions,
) -> GeometryEvaluationResult {
    if let Some(diagnostic) = validate_options(options) {
        return rejected(diagnostic);
    }
    let model = &document.model;
    let selection = match select_configuration(document, options.configuration.as_deref()) {
        Ok(selection) => selection,
        Err(diagnostic) => return rejected(diagnostic),
    };
    let topology = Topology::new(model);
    let mut losses = LossBook::default();
    let mut diagnostics = Vec::new();
    let mut partial = false;

    if selection.unresolved {
        partial = true;
        losses.record(
            "evaluation.configuration_bodies_unresolved",
            "warning",
            "configuration has unresolved body membership; the whole model was evaluated",
        );
        diagnostics.push(Diagnostic::new(
            "geometry.evaluation_configuration_unresolved",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unresolved,
            "the requested configuration has unresolved body membership; the whole model was evaluated",
        ));
    }

    let mut edge_refs = model.edges.iter().collect::<Vec<_>>();
    edge_refs.sort_by(|left, right| left.id.cmp(&right.id));
    let mut body_bounds: BTreeMap<&str, BoundsAccumulator> = model
        .bodies
        .iter()
        .map(|body| {
            let accumulator = BoundsAccumulator {
                exact: topology.body_faces_planar.get(body.id.as_str()) == Some(&true)
                    && !topology.transformed_bodies.contains(body.id.as_str()),
                ..BoundsAccumulator::default()
            };
            (body.id.as_str(), accumulator)
        })
        .collect();
    let mut model_bounds = BoundsAccumulator::default();
    let mut edges = Vec::with_capacity(edge_refs.len());
    let mut unattributed_edges = 0_u64;
    for edge in edge_refs {
        let work = evaluate_edge(edge, &topology, options, &mut losses);
        let included = match (&selection.bodies, work.output.body_id.as_deref()) {
            (None, _) => true,
            (Some(bodies), Some(body)) => bodies.contains(body),
            (Some(_), None) => {
                unattributed_edges = unattributed_edges.saturating_add(1);
                false
            }
        };
        if let Some(accumulator) = work
            .output
            .body_id
            .as_deref()
            .and_then(|body| body_bounds.get_mut(body))
        {
            accumulator.add_edge(&work);
        } else if included {
            model_bounds.add_edge(&work);
        }
        if included {
            edges.push(work.output);
        }
    }
    if unattributed_edges > 0 {
        partial = true;
        losses.record(
            "evaluation.edge_body_unresolved",
            "warning",
            "edges without a resolved owning body were excluded by the configuration filter",
        );
    }

    let mut mesh_refs = model.tessellations.iter().collect::<Vec<_>>();
    mesh_refs.sort_by(|left, right| left.id.cmp(&right.id));
    let mut meshes = Vec::with_capacity(mesh_refs.len());
    let mut invalid_triangles = 0_u64;
    let mut unattributed_meshes = 0_u64;
    for tessellation in mesh_refs {
        let evaluated = mesh::evaluate_mesh(tessellation, options.mesh_feature_angle_deg);
        let included = match (&selection.bodies, evaluated.mesh.body_id.as_deref()) {
            (None, _) => true,
            (Some(bodies), Some(body)) => bodies.contains(body),
            (Some(_), None) => {
                unattributed_meshes = unattributed_meshes.saturating_add(1);
                false
            }
        };
        if let Some(accumulator) = evaluated
            .mesh
            .body_id
            .as_deref()
            .and_then(|body| body_bounds.get_mut(body))
        {
            accumulator.add_mesh(&evaluated.mesh);
        } else if included {
            model_bounds.add_mesh(&evaluated.mesh);
        }
        if included {
            invalid_triangles = invalid_triangles.saturating_add(evaluated.invalid_triangle_count);
            if evaluated.mesh.is_closed == Some(false) {
                losses.record(
                    "evaluation.mesh_not_closed",
                    "info",
                    "meshes are not closed, consistently oriented 2-manifolds; no volume is reported",
                );
            }
            meshes.push(evaluated.mesh);
        }
    }
    if invalid_triangles > 0 {
        partial = true;
        losses.record(
            "evaluation.mesh_triangle_invalid",
            "warning",
            "mesh triangles with out-of-range or repeated vertex indices were skipped",
        );
    }
    if unattributed_meshes > 0 {
        partial = true;
        losses.record(
            "evaluation.mesh_body_unresolved",
            "warning",
            "meshes without a resolved owning body were excluded by the configuration filter",
        );
    }

    let mut transformed_included = false;
    for body in &model.bodies {
        let included = selection
            .bodies
            .as_ref()
            .is_none_or(|bodies| bodies.contains(body.id.as_str()));
        if !included {
            continue;
        }
        if topology.transformed_bodies.contains(body.id.as_str()) {
            transformed_included = true;
        }
        if let Some(accumulator) = body_bounds.get(body.id.as_str()) {
            model_bounds.merge(accumulator);
        }
    }
    if transformed_included {
        partial = true;
        losses.record(
            "evaluation.body_transform_not_applied",
            "warning",
            "bodies carry a stored transform that was not applied to sampled coordinates",
        );
    }

    let configuration_bounds = configuration_bounds(document, &body_bounds);
    let body_bounds = body_bounds
        .into_iter()
        .map(|(body_id, accumulator)| EvaluatedBodyBounds {
            body_id: body_id.to_owned(),
            bounding_box: accumulator.finish(),
        })
        .collect();

    let summary = summarize(&edges, &meshes);
    partial |= summary.unsampled_edge_count > 0 || summary.unknown_smoothness_count > 0;
    if partial {
        diagnostics.push(Diagnostic::new(
            "geometry.evaluation_partial",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unresolved,
            "some edges, normals, bodies, or meshes were withheld; see the evaluation losses",
        ));
    }
    GeometryEvaluationResult {
        status: if partial {
            GeometryEvaluationStatus::Partial
        } else {
            GeometryEvaluationStatus::Evaluated
        },
        evaluation: Some(GeometryEvaluationDocument {
            source: document.source.clone(),
            length_unit: document.length_unit.clone(),
            options: options.clone(),
            bounding_box: model_bounds.finish(),
            body_bounds,
            configuration_bounds,
            edges,
            meshes,
            summary,
            losses: losses.finish(),
        }),
        diagnostics,
    }
}

fn rejected(diagnostic: Diagnostic) -> GeometryEvaluationResult {
    GeometryEvaluationResult {
        status: GeometryEvaluationStatus::Rejected,
        evaluation: None,
        diagnostics: vec![diagnostic],
    }
}

fn validate_options(options: &GeometryEvaluationOptions) -> Option<Diagnostic> {
    let problem = if !options.chord_tolerance_mm.is_finite() || options.chord_tolerance_mm <= 0.0 {
        Some(("chord_tolerance_mm", "must be finite and positive"))
    } else if options.max_segments_per_edge == 0 {
        Some(("max_segments_per_edge", "must be at least 1"))
    } else if !options.tangent_angle_deg.is_finite()
        || !(0.0..180.0).contains(&options.tangent_angle_deg)
    {
        Some(("tangent_angle_deg", "must be finite and in [0, 180)"))
    } else if !options.mesh_feature_angle_deg.is_finite()
        || !(0.0..=180.0).contains(&options.mesh_feature_angle_deg)
    {
        Some(("mesh_feature_angle_deg", "must be finite and in [0, 180]"))
    } else {
        None
    };
    problem.map(|(field, requirement)| {
        Diagnostic::new(
            "geometry.evaluation_options_invalid",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "geometry evaluation options are invalid",
        )
        .with_detail("field", field)
        .with_detail("requirement", requirement)
    })
}

struct ConfigurationSelection {
    /// `None` evaluates the whole model.
    bodies: Option<BTreeSet<String>>,
    unresolved: bool,
}

fn select_configuration(
    document: &GeometryDocument,
    requested: Option<&str>,
) -> Result<ConfigurationSelection, Diagnostic> {
    let Some(requested) = requested else {
        return Ok(ConfigurationSelection {
            bodies: None,
            unresolved: false,
        });
    };
    let matches = document
        .configurations
        .iter()
        .filter(|state| state.name.as_deref() == Some(requested) || state.id == requested)
        .collect::<Vec<_>>();
    let state = match matches.as_slice() {
        [state] => *state,
        [] => {
            return Err(Diagnostic::new(
                "geometry.evaluation_configuration_missing",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "the requested configuration is not present in the geometry document",
            )
            .with_detail("configuration", requested));
        }
        _ => {
            return Err(Diagnostic::new(
                "geometry.evaluation_configuration_ambiguous",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "the requested configuration matches multiple configuration states",
            )
            .with_detail("configuration", requested)
            .with_detail("candidate_count", matches.len().to_string()));
        }
    };
    let Some(body_ids) = state.body_ids.as_ref() else {
        return Ok(ConfigurationSelection {
            bodies: None,
            unresolved: true,
        });
    };
    let bodies = body_ids.iter().cloned().collect::<BTreeSet<_>>();
    let all_bodies = document
        .model
        .bodies
        .iter()
        .all(|body| bodies.contains(&body.id));
    Ok(ConfigurationSelection {
        bodies: if all_bodies { None } else { Some(bodies) },
        unresolved: false,
    })
}

struct Topology<'a> {
    faces: BTreeMap<&'a str, &'a GeometryFace>,
    carriers: BTreeMap<&'a str, &'a GeometryCarrier>,
    /// Face identity per coedge use; a seam lists the same face twice.
    edge_uses: BTreeMap<&'a str, Vec<&'a str>>,
    edge_body: BTreeMap<&'a str, &'a str>,
    body_faces_planar: BTreeMap<&'a str, bool>,
    transformed_bodies: BTreeSet<&'a str>,
}

impl<'a> Topology<'a> {
    fn new(model: &'a GeometryModel) -> Self {
        let carriers = model
            .carriers
            .iter()
            .map(|carrier| (carrier.id.as_str(), carrier))
            .collect::<BTreeMap<_, _>>();
        let faces = model
            .faces
            .iter()
            .map(|face| (face.id.as_str(), face))
            .collect::<BTreeMap<_, _>>();
        let region_body = model
            .regions
            .iter()
            .map(|region| (region.id.as_str(), region.body_id.as_str()))
            .collect::<BTreeMap<_, _>>();
        let shell_body = model
            .shells
            .iter()
            .filter_map(|shell| {
                region_body
                    .get(shell.region_id.as_str())
                    .map(|body| (shell.id.as_str(), *body))
            })
            .collect::<BTreeMap<_, _>>();
        let face_body = model
            .faces
            .iter()
            .filter_map(|face| {
                shell_body
                    .get(face.shell_id.as_str())
                    .map(|body| (face.id.as_str(), *body))
            })
            .collect::<BTreeMap<_, _>>();
        let loop_face = model
            .loops
            .iter()
            .map(|item| (item.id.as_str(), item.face_id.as_str()))
            .collect::<BTreeMap<_, _>>();
        let mut edge_uses: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        let mut edge_body = BTreeMap::new();
        for coedge in &model.coedges {
            let Some(face) = loop_face.get(coedge.loop_id.as_str()) else {
                continue;
            };
            edge_uses
                .entry(coedge.edge_id.as_str())
                .or_default()
                .push(*face);
            if let Some(body) = face_body.get(*face) {
                edge_body.entry(coedge.edge_id.as_str()).or_insert(*body);
            }
        }
        for shell in &model.shells {
            if let Some(body) = shell_body.get(shell.id.as_str()) {
                for edge in &shell.wire_edge_ids {
                    edge_body.entry(edge.as_str()).or_insert(*body);
                }
            }
        }
        let mut body_faces_planar = model
            .bodies
            .iter()
            .map(|body| (body.id.as_str(), true))
            .collect::<BTreeMap<_, _>>();
        for face in &model.faces {
            let planar = carriers
                .get(face.surface_id.as_str())
                .is_some_and(|carrier| carrier.kind == "plane");
            if let Some(body) = face_body.get(face.id.as_str())
                && let Some(flag) = body_faces_planar.get_mut(*body)
            {
                *flag &= planar;
            }
        }
        let transformed_bodies = model
            .bodies
            .iter()
            .filter(|body| body.transform.is_some())
            .map(|body| body.id.as_str())
            .collect();
        Self {
            faces,
            carriers,
            edge_uses,
            edge_body,
            body_faces_planar,
            transformed_bodies,
        }
    }
}

struct EdgeWork {
    output: EvaluatedEdge,
    /// Polyline points plus analytic axis extrema for bounding boxes.
    bound_points: Vec<[f64; 3]>,
    /// Sampled from a line, circle, or ellipse with exact extrema included.
    exact: bool,
}

#[allow(clippy::too_many_lines)]
fn evaluate_edge(
    edge: &GeometryEdge,
    topology: &Topology<'_>,
    options: &GeometryEvaluationOptions,
    losses: &mut LossBook,
) -> EdgeWork {
    let (interval_source, range) = if let Some(range) = edge.parameter_range {
        (EdgeIntervalSource::Source, Some(range))
    } else if let Some(derived) = &edge.derived_parameter_interval {
        (EdgeIntervalSource::Derived, Some(derived.parameter_range))
    } else {
        (EdgeIntervalSource::Unavailable, None)
    };
    let carrier = edge
        .curve_id
        .as_deref()
        .and_then(|id| topology.carriers.get(id).copied());
    let curve_kind = carrier.map_or_else(|| "missing".to_owned(), |carrier| carrier.kind.clone());
    let curve = carrier.map(|carrier| carriers::curve_from_definition(&carrier.definition));

    let mut reason = None;
    let mut sampled = None;
    match (range, curve) {
        (_, None) => {
            reason = Some("curve_missing");
            losses.record(
                "evaluation.edge_curve_missing",
                "warning",
                "edges reference no decoded curve carrier",
            );
        }
        (_, Some(None)) => {
            reason = Some("definition_unreadable");
            losses.record(
                "evaluation.curve_definition_unreadable",
                "warning",
                "edge curve definitions could not be read back into an evaluable carrier",
            );
        }
        (None, Some(Some(_))) => {
            reason = Some("interval_unavailable");
            losses.record(
                "evaluation.edge_interval_unavailable",
                "warning",
                "edges have neither a source parameter range nor a derived interval",
            );
        }
        (Some(range), Some(Some(curve))) => {
            match carriers::sample_curve(
                &curve,
                range,
                options.chord_tolerance_mm,
                options.max_segments_per_edge,
            ) {
                Ok(result) => sampled = Some((curve, result)),
                Err(failure) => {
                    reason = Some(failure.reason());
                    losses.record(
                        failure.loss_code(),
                        "warning",
                        "edges could not be sampled from their curve carrier",
                    );
                }
            }
        }
    }

    let uses = topology
        .edge_uses
        .get(edge.id.as_str())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let adjacent_face_ids = uses
        .iter()
        .map(|face| (*face).to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let (smoothness, dihedral_angle_deg) = match (&sampled, uses) {
        (None, _) => (EdgeSmoothness::Unknown, None),
        (Some(_), [_]) => (EdgeSmoothness::Boundary, None),
        (Some((curve, _)), [first, second]) => {
            if let Some(angle) = dihedral_angle(curve, range, [*first, *second], edge, topology) {
                let smoothness = if angle <= options.tangent_angle_deg {
                    EdgeSmoothness::Tangent
                } else {
                    EdgeSmoothness::Sharp
                };
                (smoothness, Some(angle))
            } else {
                losses.record(
                    "evaluation.surface_normal_unavailable",
                    "warning",
                    "edges have an adjacent face whose surface normal could not be established",
                );
                (EdgeSmoothness::Unknown, None)
            }
        }
        (Some(_), _) => {
            losses.record(
                "evaluation.edge_use_count_unsupported",
                "warning",
                "edges have no face use or more than two face uses",
            );
            (EdgeSmoothness::Unknown, None)
        }
    };

    let (polyline, deviation, analytic) = match &sampled {
        Some((_, result)) => (
            result.points.clone(),
            Some(result.deviation_mm),
            result.analytic,
        ),
        None => (Vec::new(), None, false),
    };
    let mut bound_points = polyline.clone();
    if let (Some((curve, _)), Some(range)) = (&sampled, range)
        && analytic
    {
        bound_points.extend(carriers::conic_axis_extrema(curve, range));
    }
    let segment_count = u32::try_from(polyline.len().saturating_sub(1)).unwrap_or(u32::MAX);
    EdgeWork {
        output: EvaluatedEdge {
            edge_id: edge.id.clone(),
            curve_id: edge.curve_id.clone(),
            curve_kind,
            body_id: topology
                .edge_body
                .get(edge.id.as_str())
                .map(|body| (*body).to_owned()),
            interval_source,
            parameter_range: range,
            status: if sampled.is_some() {
                EdgeSampleStatus::Sampled
            } else {
                EdgeSampleStatus::Unsampled
            },
            reason: reason.map(str::to_owned),
            polyline,
            segment_count,
            chord_tolerance_mm: deviation,
            adjacent_face_ids,
            smoothness,
            dihedral_angle_deg,
            is_derived_seam: edge.provenance.tag.as_deref() == Some(DERIVED_SEAM_TAG),
        },
        bound_points,
        exact: sampled.is_some() && analytic,
    }
}

/// Unsigned angle in degrees between the outward normals of two face uses at
/// the edge's parameter midpoint. Face sense flips the carrier normal.
fn dihedral_angle(
    curve: &CurveGeometry,
    range: Option<[f64; 2]>,
    uses: [&str; 2],
    edge: &GeometryEdge,
    topology: &Topology<'_>,
) -> Option<f64> {
    let range = range?;
    let midpoint = eval::curve_point(curve, range[0] + (range[1] - range[0]) / 2.0)?;
    let mut normals = Vec::with_capacity(2);
    for face_id in uses {
        let face = topology.faces.get(face_id)?;
        let carrier = topology.carriers.get(face.surface_id.as_str())?;
        let surface = carriers::surface_from_definition(&carrier.definition)?;
        let tolerance = SURFACE_FIT_TOLERANCE_MM
            .max(edge.tolerance.unwrap_or(0.0))
            .max(face.tolerance.unwrap_or(0.0));
        normals.push(outward_normal(&surface, face, midpoint, tolerance)?);
    }
    let cosine = normals[0].dot(normals[1]).clamp(-1.0, 1.0);
    let angle = cosine.acos().to_degrees();
    angle.is_finite().then_some(angle)
}

fn outward_normal(
    surface: &SurfaceGeometry,
    face: &GeometryFace,
    point: Point3,
    tolerance: f64,
) -> Option<Vector3> {
    let normal = carriers::surface_normal_at(surface, point, tolerance)?;
    match face.sense.as_str() {
        "forward" => Some(normal),
        "reversed" => Some(normal.scale(-1.0)),
        _ => None,
    }
}

#[derive(Clone)]
struct BoundsAccumulator {
    min: [f64; 3],
    max: [f64; 3],
    edge_count: u64,
    vertex_count: u64,
    has_edge_points: bool,
    has_mesh: bool,
    exact: bool,
}

impl Default for BoundsAccumulator {
    fn default() -> Self {
        Self {
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
            edge_count: 0,
            vertex_count: 0,
            has_edge_points: false,
            has_mesh: false,
            exact: true,
        }
    }
}

impl BoundsAccumulator {
    fn add_point(&mut self, point: [f64; 3]) {
        if point.iter().any(|value| !value.is_finite()) {
            self.exact = false;
            return;
        }
        for ((low, high), value) in self.min.iter_mut().zip(self.max.iter_mut()).zip(point) {
            *low = low.min(value);
            *high = high.max(value);
        }
    }

    fn add_edge(&mut self, work: &EdgeWork) {
        if work.output.status == EdgeSampleStatus::Unsampled {
            self.exact = false;
            return;
        }
        self.exact &= work.exact;
        self.edge_count = self.edge_count.saturating_add(1);
        for point in &work.bound_points {
            self.has_edge_points = true;
            self.add_point(*point);
        }
    }

    fn add_mesh(&mut self, mesh: &EvaluatedMesh) {
        let Some(bounds) = mesh.bounds else {
            return;
        };
        self.has_mesh = true;
        self.vertex_count = self.vertex_count.saturating_add(mesh.vertex_count);
        self.add_point(bounds.min);
        self.add_point(bounds.max);
    }

    fn merge(&mut self, other: &Self) {
        if other.has_edge_points || other.has_mesh {
            self.add_point(other.min);
            self.add_point(other.max);
        }
        self.edge_count = self.edge_count.saturating_add(other.edge_count);
        self.vertex_count = self.vertex_count.saturating_add(other.vertex_count);
        self.has_edge_points |= other.has_edge_points;
        self.has_mesh |= other.has_mesh;
        self.exact &= other.exact;
    }

    fn finish(self) -> Option<EvaluatedBoundingBox> {
        let method = match (self.has_edge_points, self.has_mesh) {
            (true, true) => BoundingBoxMethod::EdgeSamplesAndTessellation,
            (true, false) => BoundingBoxMethod::EdgeSamples,
            (false, true) => BoundingBoxMethod::Tessellation,
            (false, false) => return None,
        };
        if self
            .min
            .iter()
            .chain(self.max.iter())
            .any(|value| !value.is_finite())
        {
            return None;
        }
        Some(EvaluatedBoundingBox {
            bounds: EvaluatedBounds {
                min: self.min,
                max: self.max,
            },
            method,
            exact: self.has_edge_points && self.exact,
            edge_count: self.edge_count,
            vertex_count: self.vertex_count,
        })
    }
}

fn configuration_bounds(
    document: &GeometryDocument,
    body_bounds: &BTreeMap<&str, BoundsAccumulator>,
) -> Vec<EvaluatedConfigurationBounds> {
    let mut states = document.configurations.iter().collect::<Vec<_>>();
    states.sort_by(|left, right| left.id.cmp(&right.id));
    states
        .into_iter()
        .map(|state| {
            let bounding_box = state.body_ids.as_ref().and_then(|body_ids| {
                let mut accumulator = BoundsAccumulator::default();
                let mut found = false;
                for body_id in body_ids {
                    if let Some(bounds) = body_bounds.get(body_id.as_str()) {
                        found = true;
                        accumulator.merge(bounds);
                    }
                }
                found.then(|| accumulator.finish()).flatten()
            });
            EvaluatedConfigurationBounds {
                configuration_id: state.id.clone(),
                name: state.name.clone(),
                body_ids: state.body_ids.clone(),
                bounding_box,
            }
        })
        .collect()
}

fn summarize(edges: &[EvaluatedEdge], meshes: &[EvaluatedMesh]) -> GeometryEvaluationSummary {
    let count = |predicate: &dyn Fn(&EvaluatedEdge) -> bool| {
        u64::try_from(edges.iter().filter(|edge| predicate(edge)).count()).unwrap_or(u64::MAX)
    };
    GeometryEvaluationSummary {
        edge_count: u64::try_from(edges.len()).unwrap_or(u64::MAX),
        sampled_edge_count: count(&|edge| edge.status == EdgeSampleStatus::Sampled),
        unsampled_edge_count: count(&|edge| edge.status == EdgeSampleStatus::Unsampled),
        sharp_edge_count: count(&|edge| edge.smoothness == EdgeSmoothness::Sharp),
        tangent_edge_count: count(&|edge| edge.smoothness == EdgeSmoothness::Tangent),
        boundary_edge_count: count(&|edge| edge.smoothness == EdgeSmoothness::Boundary),
        unknown_smoothness_count: count(&|edge| edge.smoothness == EdgeSmoothness::Unknown),
        mesh_count: u64::try_from(meshes.len()).unwrap_or(u64::MAX),
        closed_mesh_count: u64::try_from(
            meshes
                .iter()
                .filter(|mesh| mesh.is_closed == Some(true))
                .count(),
        )
        .unwrap_or(u64::MAX),
    }
}

/// Aggregates repeated conditions into one loss per code with a count, so a
/// model with thousands of withheld edges stays bounded in size.
#[derive(Default)]
struct LossBook {
    entries: BTreeMap<&'static str, (u64, &'static str, &'static str)>,
}

impl LossBook {
    fn record(&mut self, code: &'static str, severity: &'static str, message: &'static str) {
        let entry = self.entries.entry(code).or_insert((0, severity, message));
        entry.0 = entry.0.saturating_add(1);
    }

    fn finish(self) -> Vec<GeometryLoss> {
        self.entries
            .into_iter()
            .map(|(code, (count, severity, message))| GeometryLoss {
                code: code.to_owned(),
                taxonomy: LOSS_TAXONOMY.to_owned(),
                category: "evaluation".to_owned(),
                severity: severity.to_owned(),
                message: format!("{count} occurrence(s): {message}"),
                stream: None,
                offset: None,
                tag: None,
            })
            .collect()
    }
}
