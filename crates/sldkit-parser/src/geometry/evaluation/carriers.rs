//! Sampling and normal evaluation of public carrier definitions.
//!
//! Definitions are the serialized neutral carriers, so they round-trip into
//! the shared evaluators without a second parser. Nothing here replaces a
//! stored carrier, trim field, or tessellation.
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use cadmpeg_ir::{
    eval,
    geometry::{CurveGeometry, SurfaceGeometry},
    math::{Point3, Vector3},
};
use serde_json::Value;

/// Maximum residual accepted when a point is re-evaluated on its surface.
/// Parasolid's default linear precision is `1e-8 m`, which is `1e-5 mm`.
pub(super) const SURFACE_FIT_TOLERANCE_MM: f64 = 1.0e-5;
const MAX_REFINEMENT_SEGMENTS: usize = 1 << 16;

pub(super) fn curve_from_definition(definition: &Value) -> Option<CurveGeometry> {
    serde_json::from_value(definition.clone()).ok()
}

pub(super) fn surface_from_definition(definition: &Value) -> Option<SurfaceGeometry> {
    serde_json::from_value(definition.clone()).ok()
}

/// Ordered polyline plus a bound on its deviation from the exact curve.
pub(super) struct Sampled {
    pub points: Vec<[f64; 3]>,
    pub deviation_mm: f64,
    /// Lines, circles, and ellipses contribute exact axis extrema; see
    /// [`conic_axis_extrema`].
    pub analytic: bool,
}

/// Stable machine-readable reasons; they are copied into `EvaluatedEdge::reason`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SampleFailure {
    DegenerateCurve,
    CurveKindUnsupported,
    EmptyInterval,
    EvaluationFailed,
}

impl SampleFailure {
    pub(super) const fn reason(self) -> &'static str {
        match self {
            Self::DegenerateCurve => "degenerate_curve",
            Self::CurveKindUnsupported => "curve_kind_unsupported",
            Self::EmptyInterval => "empty_interval",
            Self::EvaluationFailed => "evaluation_failed",
        }
    }

    pub(super) const fn loss_code(self) -> &'static str {
        match self {
            Self::DegenerateCurve | Self::CurveKindUnsupported => {
                "evaluation.curve_kind_unsupported"
            }
            Self::EmptyInterval => "evaluation.edge_interval_empty",
            Self::EvaluationFailed => "evaluation.curve_evaluation_failed",
        }
    }
}

fn finite_point(point: Point3) -> Option<Point3> {
    (point.x.is_finite() && point.y.is_finite() && point.z.is_finite()).then_some(point)
}

fn array(point: Point3) -> [f64; 3] {
    [point.x, point.y, point.z]
}

fn evaluate(curve: &CurveGeometry, t: f64) -> Option<Point3> {
    eval::curve_point(curve, t).and_then(finite_point)
}

/// Sample `curve` over the directed `range` so that no polyline point deviates
/// from the exact curve by more than `tolerance` where the segment budget permits.
// An empty interval is an exact structural fact, not approximate geometry.
#[allow(clippy::float_cmp)]
pub(super) fn sample_curve(
    curve: &CurveGeometry,
    range: [f64; 2],
    tolerance: f64,
    max_segments: u32,
) -> Result<Sampled, SampleFailure> {
    if range.iter().any(|value| !value.is_finite()) {
        return Err(SampleFailure::EvaluationFailed);
    }
    if range[0] == range[1] {
        return Err(SampleFailure::EmptyInterval);
    }
    match curve {
        CurveGeometry::Line { .. } => {
            let points = [evaluate(curve, range[0]), evaluate(curve, range[1])];
            let (Some(start), Some(end)) = (points[0], points[1]) else {
                return Err(SampleFailure::EvaluationFailed);
            };
            Ok(Sampled {
                points: vec![array(start), array(end)],
                deviation_mm: 0.0,
                analytic: true,
            })
        }
        CurveGeometry::Circle { radius, .. } => {
            sample_conic(curve, range, *radius, tolerance, max_segments)
        }
        CurveGeometry::Ellipse { major_radius, .. } => {
            sample_conic(curve, range, *major_radius, tolerance, max_segments)
        }
        CurveGeometry::Degenerate { .. } => Err(SampleFailure::DegenerateCurve),
        CurveGeometry::Unknown { .. }
        | CurveGeometry::Procedural { .. }
        | CurveGeometry::Composite { .. } => Err(SampleFailure::CurveKindUnsupported),
        _ => sample_adaptive(curve, range, tolerance, max_segments),
    }
}

/// Uniform angular subdivision; the chord deviation of an elliptic arc is
/// bounded by the same sagitta formula scaled by the semi-major radius.
fn sample_conic(
    curve: &CurveGeometry,
    range: [f64; 2],
    radius: f64,
    tolerance: f64,
    max_segments: u32,
) -> Result<Sampled, SampleFailure> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(SampleFailure::EvaluationFailed);
    }
    let span = range[1] - range[0];
    let segments = conic_segments(span.abs(), radius, tolerance, max_segments);
    let mut points = Vec::with_capacity(usize::try_from(segments).unwrap_or(1).saturating_add(1));
    for index in 0..=segments {
        let t = range[0] + span * f64::from(index) / f64::from(segments);
        let point = evaluate(curve, t).ok_or(SampleFailure::EvaluationFailed)?;
        points.push(array(point));
    }
    let step = span.abs() / f64::from(segments);
    let deviation_mm = if step >= PI {
        radius
    } else {
        radius * (1.0 - (step / 2.0).cos())
    };
    Ok(Sampled {
        points,
        deviation_mm,
        analytic: true,
    })
}

fn conic_segments(span: f64, radius: f64, tolerance: f64, max_segments: u32) -> u32 {
    let step = if tolerance >= radius {
        FRAC_PI_2
    } else {
        (2.0 * (1.0 - tolerance / radius).acos()).min(FRAC_PI_2)
    };
    if !step.is_finite() || step <= 0.0 {
        return max_segments.max(1);
    }
    segment_count((span / step).ceil(), max_segments)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn segment_count(value: f64, max_segments: u32) -> u32 {
    if !value.is_finite() || value < 1.0 {
        return 1;
    }
    value.min(f64::from(max_segments.max(1))) as u32
}

struct Segment {
    t0: f64,
    t1: f64,
    p0: Point3,
    p1: Point3,
    deviation: f64,
}

fn segment(curve: &CurveGeometry, t0: f64, p0: Point3, t1: f64, p1: Point3) -> Option<Segment> {
    let mut deviation = 0.0_f64;
    for fraction in [0.25, 0.5, 0.75] {
        let t = t0 + (t1 - t0) * fraction;
        let point = evaluate(curve, t)?;
        deviation = deviation.max(chord_distance(point, p0, p1));
    }
    deviation.is_finite().then_some(Segment {
        t0,
        t1,
        p0,
        p1,
        deviation,
    })
}

/// Refine the segment with the largest deviation first, so an exhausted budget
/// degrades the whole polyline evenly instead of one end of it.
// A midpoint equal to an endpoint means the parameter interval is exhausted.
#[allow(clippy::float_cmp)]
fn sample_adaptive(
    curve: &CurveGeometry,
    range: [f64; 2],
    tolerance: f64,
    max_segments: u32,
) -> Result<Sampled, SampleFailure> {
    let start = evaluate(curve, range[0]).ok_or(SampleFailure::EvaluationFailed)?;
    let end = evaluate(curve, range[1]).ok_or(SampleFailure::EvaluationFailed)?;
    let budget = usize::try_from(max_segments.max(1))
        .unwrap_or(MAX_REFINEMENT_SEGMENTS)
        .min(MAX_REFINEMENT_SEGMENTS);
    let mut segments = vec![
        segment(curve, range[0], start, range[1], end).ok_or(SampleFailure::EvaluationFailed)?,
    ];
    while segments.len() < budget {
        let Some((index, _)) = segments
            .iter()
            .enumerate()
            .filter(|(_, item)| item.deviation > tolerance)
            .max_by(|(left_index, left), (right_index, right)| {
                left.deviation
                    .total_cmp(&right.deviation)
                    .then_with(|| right_index.cmp(left_index))
            })
        else {
            break;
        };
        let item = &segments[index];
        let middle = item.t0 + (item.t1 - item.t0) / 2.0;
        if middle == item.t0 || middle == item.t1 {
            break;
        }
        let point = evaluate(curve, middle).ok_or(SampleFailure::EvaluationFailed)?;
        let left = segment(curve, item.t0, item.p0, middle, point)
            .ok_or(SampleFailure::EvaluationFailed)?;
        let right = segment(curve, middle, point, item.t1, item.p1)
            .ok_or(SampleFailure::EvaluationFailed)?;
        segments[index] = left;
        segments.insert(index + 1, right);
    }
    let mut points = Vec::with_capacity(segments.len() + 1);
    points.push(array(start));
    let mut deviation_mm = 0.0_f64;
    for item in &segments {
        points.push(array(item.p1));
        deviation_mm = deviation_mm.max(item.deviation);
    }
    Ok(Sampled {
        points,
        deviation_mm,
        analytic: false,
    })
}

/// Distance from `point` to the closed segment between `start` and `end`.
fn chord_distance(point: Point3, start: Point3, end: Point3) -> f64 {
    let chord = end.vector_from(start);
    let length_squared = chord.dot(chord);
    if length_squared <= 0.0 {
        return point.distance(start);
    }
    let fraction = (point.vector_from(start).dot(chord) / length_squared).clamp(0.0, 1.0);
    point.distance(start.translated(chord, fraction))
}

/// Exact axis-aligned extrema of a circular or elliptic arc inside `range`.
/// Other curve kinds contribute no extrema; their samples bound them only
/// approximately.
// A coordinate with no angular dependence has no extremum to add.
#[allow(clippy::float_cmp)]
pub(super) fn conic_axis_extrema(curve: &CurveGeometry, range: [f64; 2]) -> Vec<[f64; 3]> {
    let (axis, major_direction, major, minor) = match curve {
        CurveGeometry::Circle {
            axis,
            ref_direction,
            radius,
            ..
        } => (*axis, *ref_direction, *radius, *radius),
        CurveGeometry::Ellipse {
            axis,
            major_direction,
            major_radius,
            minor_radius,
            ..
        } => (*axis, *major_direction, *major_radius, *minor_radius),
        _ => return Vec::new(),
    };
    let minor_direction = axis.cross(major_direction);
    let low = range[0].min(range[1]);
    let high = range[0].max(range[1]);
    if !low.is_finite() || !high.is_finite() {
        return Vec::new();
    }
    let mut extrema = Vec::new();
    for coordinate in 0..3 {
        let cosine = major * component(major_direction, coordinate);
        let sine = minor * component(minor_direction, coordinate);
        if cosine == 0.0 && sine == 0.0 {
            continue;
        }
        let base = sine.atan2(cosine);
        for phase in [base, base + PI] {
            // Every representative of the critical angle inside the interval.
            let first = phase + ((low - phase) / TAU).ceil() * TAU;
            let mut t = first;
            let mut guard = 0;
            while t <= high && guard < 8 {
                if let Some(point) = evaluate(curve, t) {
                    extrema.push(array(point));
                }
                t += TAU;
                guard += 1;
            }
        }
    }
    extrema
}

fn component(vector: Vector3, index: usize) -> f64 {
    match index {
        0 => vector.x,
        1 => vector.y,
        _ => vector.z,
    }
}

/// Unit normal `du × dv` of `surface` at a point known to lie on it. Analytic
/// surfaces are inverted in closed form; NURBS surfaces through the shared
/// bounded solver. Procedural, transformed, polygonal, and unknown carriers
/// return `None` so that callers report the normal as unavailable.
pub(super) fn surface_normal_at(
    surface: &SurfaceGeometry,
    point: Point3,
    tolerance: f64,
) -> Option<Vector3> {
    let parameters = match surface {
        SurfaceGeometry::Plane { .. }
        | SurfaceGeometry::Cylinder { .. }
        | SurfaceGeometry::Cone { .. }
        | SurfaceGeometry::Sphere { .. }
        | SurfaceGeometry::Torus { .. } => eval::analytic_surface_parameters(surface, point)?,
        SurfaceGeometry::Nurbs(nurbs) => {
            eval::nurbs_surface_parameter_within_tolerance(nurbs, point, None, tolerance)?
        }
        _ => return None,
    };
    if !parameters.u.is_finite() || !parameters.v.is_finite() {
        return None;
    }
    let partials = eval::surface_partials(surface, parameters.u, parameters.v)?;
    let residual = finite_point(partials.point)?.distance(point);
    if !residual.is_finite() || residual > tolerance {
        return None;
    }
    let normal = partials.du.cross(partials.dv).unit()?;
    (normal.x.is_finite() && normal.y.is_finite() && normal.z.is_finite()).then_some(normal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadmpeg_ir::geometry::NurbsCurve;

    fn circle() -> CurveGeometry {
        CurveGeometry::Circle {
            center: Point3::new(1.0, 2.0, 3.0),
            axis: Vector3::new(0.0, 0.0, 1.0),
            ref_direction: Vector3::new(1.0, 0.0, 0.0),
            radius: 5.0,
        }
    }

    #[test]
    fn circle_samples_respect_the_chord_tolerance_and_endpoints() -> Result<(), &'static str> {
        let curve = circle();
        let sampled =
            sample_curve(&curve, [0.0, PI], 0.01, 512).map_err(|_| "circle must sample")?;
        assert!(sampled.analytic);
        assert!(sampled.deviation_mm <= 0.01);
        assert!(sampled.points.len() > 10);
        assert!((sampled.points[0][0] - 6.0).abs() < 1e-12);
        let last = sampled.points[sampled.points.len() - 1];
        assert!((last[0] + 4.0).abs() < 1e-9 && (last[1] - 2.0).abs() < 1e-9);
        for pair in sampled.points.windows(2) {
            let mid = [
                f64::midpoint(pair[0][0], pair[1][0]),
                f64::midpoint(pair[0][1], pair[1][1]),
            ];
            let radial = (mid[0] - 1.0).hypot(mid[1] - 2.0);
            assert!(5.0 - radial <= 0.01 + 1e-12);
        }
        let coarse = sample_curve(&curve, [0.0, TAU], 0.01, 4).map_err(|_| "budget")?;
        assert_eq!(coarse.points.len(), 5);
        assert!(coarse.deviation_mm > 0.01);
        Ok(())
    }

    #[test]
    fn conic_extrema_cover_only_the_sampled_interval() {
        let extrema = conic_axis_extrema(&circle(), [0.0, PI]);
        let max_y = extrema
            .iter()
            .map(|point| point[1])
            .fold(f64::NEG_INFINITY, f64::max);
        let min_y = extrema
            .iter()
            .map(|point| point[1])
            .fold(f64::INFINITY, f64::min);
        assert!((max_y - 7.0).abs() < 1e-12);
        assert!(min_y >= 2.0 - 1e-12);
        assert!(conic_axis_extrema(&circle(), [f64::NAN, 1.0]).is_empty());
    }

    // Support endpoints are reproduced exactly by clamped knots.
    #[test]
    #[allow(clippy::float_cmp)]
    fn nurbs_samples_start_and_end_on_the_support_endpoints() -> Result<(), &'static str> {
        let curve = CurveGeometry::Nurbs(NurbsCurve {
            degree: 3,
            knots: vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0],
            control_points: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(10.0, 30.0, 0.0),
                Point3::new(20.0, -30.0, 0.0),
                Point3::new(30.0, 30.0, 0.0),
                Point3::new(40.0, 0.0, 0.0),
            ],
            weights: None,
            periodic: false,
        });
        let sampled = sample_curve(&curve, [0.0, 1.0], 0.01, 512).map_err(|_| "nurbs")?;
        assert!(!sampled.analytic);
        assert_eq!(sampled.points[0], [0.0, 0.0, 0.0]);
        assert_eq!(sampled.points[sampled.points.len() - 1], [40.0, 0.0, 0.0]);
        assert!(sampled.deviation_mm <= 0.01);
        assert!(sampled.points.len() > 20);
        let reversed = sample_curve(&curve, [1.0, 0.0], 0.01, 512).map_err(|_| "nurbs")?;
        assert_eq!(reversed.points[0], [40.0, 0.0, 0.0]);
        let limited = sample_curve(&curve, [0.0, 1.0], 0.01, 3).map_err(|_| "nurbs")?;
        assert_eq!(limited.points.len(), 4);
        assert!(limited.deviation_mm > 0.01);
        Ok(())
    }

    #[test]
    fn unsupported_and_empty_inputs_fail_closed() {
        let degenerate = CurveGeometry::Degenerate {
            point: Point3::new(0.0, 0.0, 0.0),
        };
        assert_eq!(
            sample_curve(&degenerate, [0.0, 1.0], 0.01, 8).err(),
            Some(SampleFailure::DegenerateCurve)
        );
        assert_eq!(
            sample_curve(&circle(), [1.0, 1.0], 0.01, 8).err(),
            Some(SampleFailure::EmptyInterval)
        );
        assert_eq!(
            sample_curve(&circle(), [0.0, f64::NAN], 0.01, 8).err(),
            Some(SampleFailure::EvaluationFailed)
        );
        let unknown = CurveGeometry::Unknown { record: None };
        assert_eq!(
            sample_curve(&unknown, [0.0, 1.0], 0.01, 8).err(),
            Some(SampleFailure::CurveKindUnsupported)
        );
    }

    #[test]
    fn analytic_and_nurbs_surface_normals_are_unit_and_outward_for_du_cross_dv()
    -> Result<(), &'static str> {
        let cylinder = SurfaceGeometry::Cylinder {
            origin: Point3::new(0.0, 0.0, 0.0),
            axis: Vector3::new(0.0, 0.0, 1.0),
            ref_direction: Vector3::new(1.0, 0.0, 0.0),
            radius: 2.0,
        };
        let normal = surface_normal_at(
            &cylinder,
            Point3::new(0.0, 2.0, 7.0),
            SURFACE_FIT_TOLERANCE_MM,
        )
        .ok_or("cylinder normal")?;
        assert!((normal.x).abs() < 1e-12 && (normal.y - 1.0).abs() < 1e-12);
        assert!(
            surface_normal_at(
                &cylinder,
                Point3::new(0.0, 2.5, 7.0),
                SURFACE_FIT_TOLERANCE_MM
            )
            .is_none()
        );
        let plane = SurfaceGeometry::Plane {
            origin: Point3::new(0.0, 0.0, 5.0),
            normal: Vector3::new(0.0, 0.0, 1.0),
            u_axis: Vector3::new(1.0, 0.0, 0.0),
        };
        let normal = surface_normal_at(
            &plane,
            Point3::new(3.0, -4.0, 5.0),
            SURFACE_FIT_TOLERANCE_MM,
        )
        .ok_or("plane normal")?;
        assert!((normal.z - 1.0).abs() < 1e-12);
        let sheet = SurfaceGeometry::Nurbs(cadmpeg_ir::geometry::NurbsSurface {
            u_degree: 1,
            v_degree: 1,
            u_knots: vec![0.0, 0.0, 1.0, 1.0],
            v_knots: vec![0.0, 0.0, 1.0, 1.0],
            u_count: 2,
            v_count: 2,
            control_points: vec![
                Point3::new(0.0, 0.0, 20.0),
                Point3::new(0.0, 10.0, 20.0),
                Point3::new(10.0, 0.0, 20.0),
                Point3::new(10.0, 10.0, 20.0),
            ],
            weights: None,
            u_periodic: false,
            v_periodic: false,
        });
        let normal = surface_normal_at(
            &sheet,
            Point3::new(5.0, 0.0, 20.0),
            SURFACE_FIT_TOLERANCE_MM,
        )
        .ok_or("nurbs normal")?;
        assert!((normal.z.abs() - 1.0).abs() < 1e-12);
        Ok(())
    }
}
