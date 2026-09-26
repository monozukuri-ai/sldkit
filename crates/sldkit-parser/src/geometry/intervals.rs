//! Bounded endpoint-derived intervals; never overwrite native trim metadata.
use std::collections::{BTreeMap, BTreeSet};

use cadmpeg_ir::{CadIr, geometry::CurveGeometry, math::Point3, topology::Edge};
use sldkit_core::{GeometryDerivedInterval, GeometryIntervalMethod};

const TOLERANCE_MM: f64 = 1.0e-7;

pub(super) struct Sources<'a> {
    curves: BTreeMap<&'a str, &'a CurveGeometry>,
    vertices: BTreeMap<&'a str, Point3>,
    annotations: &'a cadmpeg_ir::Annotations,
    native_streams: BTreeSet<u32>,
}

impl<'a> Sources<'a> {
    pub(super) fn new(ir: &'a CadIr, annotations: &'a cadmpeg_ir::Annotations) -> Self {
        let points: BTreeMap<_, _> = ir
            .model
            .points
            .iter()
            .map(|point| (point.id.as_str(), point.position))
            .collect();
        Self {
            annotations,
            native_streams: ir
                .model
                .bodies
                .iter()
                .filter_map(|body| {
                    let p = annotations.provenance.get(body.id.as_str())?;
                    (p.tag.as_deref() == Some("00_0c_body")).then_some(p.stream)
                })
                .collect(),
            curves: ir
                .model
                .curves
                .iter()
                .map(|curve| (curve.id.as_str(), &curve.geometry))
                .collect(),
            vertices: ir
                .model
                .vertices
                .iter()
                .filter_map(|vertex| {
                    Some((vertex.id.as_str(), *points.get(vertex.point.as_str())?))
                })
                .collect(),
        }
    }

    pub(super) fn native_entity(&self, id: &str) -> bool {
        self.annotations
            .provenance
            .get(id)
            .is_some_and(|p| self.native_streams.contains(&p.stream))
    }
}

pub(super) fn derive_for_edge(
    edge: &Edge,
    sources: &Sources<'_>,
) -> Option<GeometryDerivedInterval> {
    if edge.param_range.is_some() {
        return None;
    }
    let curve = sources.curves.get(edge.curve.as_ref()?.as_str())?;
    let endpoints = [
        *sources.vertices.get(edge.start.as_str())?,
        *sources.vertices.get(edge.end.as_str())?,
    ];
    if matches!(
        curve,
        CurveGeometry::Circle { .. } | CurveGeometry::Ellipse { .. }
    ) {
        if !sources.native_entity(edge.id.as_str()) {
            return None;
        }
        let closed_seam = edge.start == edge.end
            && sources
                .annotations
                .provenance
                .get(edge.start.as_str())
                .is_some_and(|p| p.tag.as_deref() == Some("derived_closed_circle_seam"));
        return conic_interval(curve, endpoints, closed_seam);
    }
    derive(curve, endpoints).or_else(|| {
        let CurveGeometry::Nurbs(nurbs) = curve else {
            return None;
        };
        let (parameter_range, error) = super::nurbs_intervals::interval(nurbs, endpoints)?;
        Some(GeometryDerivedInterval {
            parameter_range,
            method: GeometryIntervalMethod::NurbsMonotoneProjection,
            tolerance_mm: TOLERANCE_MM,
            max_endpoint_error_mm: error,
        })
    })
}

/// Native FIN/edge direction determines the positive arc, including arcs longer
/// than pi. Endpoint coincidence alone never licenses a full-period interval.
fn conic_interval(
    curve: &CurveGeometry,
    endpoints: [Point3; 2],
    closed_seam: bool,
) -> Option<GeometryDerivedInterval> {
    use std::f64::consts::TAU;
    let (center, axis, x, major, minor) = match curve {
        CurveGeometry::Circle {
            center,
            axis,
            ref_direction,
            radius,
        } => (*center, *axis, *ref_direction, *radius, *radius),
        CurveGeometry::Ellipse {
            center,
            axis,
            major_direction,
            major_radius,
            minor_radius,
        } if !closed_seam => (
            *center,
            *axis,
            *major_direction,
            *major_radius,
            *minor_radius,
        ),
        _ => return None,
    };
    if !major.is_finite()
        || !minor.is_finite()
        || [
            center.x, center.y, center.z, axis.x, axis.y, axis.z, x.x, x.y, x.z,
        ]
        .iter()
        .any(|v| !v.is_finite())
        || endpoints
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
        || minor <= 0.0
        || major < minor
        || (axis.norm() - 1.0).abs() > 1e-9
        || (x.norm() - 1.0).abs() > 1e-9
        || (axis.x * x.x + axis.y * x.y + axis.z * x.z).abs() > 1e-9
        || (!closed_seam && distance(endpoints[0], endpoints[1]) <= 2.0 * TOLERANCE_MM)
    {
        return None;
    }
    let y = axis.cross(x);
    let angles = endpoints.map(|p| {
        let d = [p.x - center.x, p.y - center.y, p.z - center.z];
        let cosine = (d[0] * x.x + d[1] * x.y + d[2] * x.z) / major;
        let sine = (d[0] * y.x + d[1] * y.y + d[2] * y.z) / minor;
        sine.atan2(cosine).rem_euclid(TAU)
    });
    let span = if closed_seam {
        TAU
    } else {
        (angles[1] - angles[0]).rem_euclid(TAU)
    };
    let range = [angles[0], angles[0] + span];
    if range.iter().any(|v| !v.is_finite()) || span <= 0.0 {
        return None;
    }
    let error = distance(
        cadmpeg_ir::eval::curve_point(curve, range[0])?,
        endpoints[0],
    )
    .max(distance(
        cadmpeg_ir::eval::curve_point(curve, range[1])?,
        endpoints[1],
    ));
    if !error.is_finite() || error > TOLERANCE_MM {
        return None;
    }
    Some(GeometryDerivedInterval {
        parameter_range: range,
        method: if closed_seam {
            GeometryIntervalMethod::ClosedCircleSeam
        } else {
            GeometryIntervalMethod::ConicEndpoints
        },
        tolerance_mm: TOLERANCE_MM,
        max_endpoint_error_mm: error,
    })
}

fn distance(a: Point3, b: Point3) -> f64 {
    (a.x - b.x).hypot(a.y - b.y).hypot(a.z - b.z)
}

// Knot multiplicity and empty parameter domains require exact comparisons.
#[allow(clippy::float_cmp)]
fn derive(curve: &CurveGeometry, endpoints: [Point3; 2]) -> Option<GeometryDerivedInterval> {
    if endpoints
        .iter()
        .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
        || distance(endpoints[0], endpoints[1]) <= 2.0 * TOLERANCE_MM
    {
        return None;
    }
    let (mut range, method) = match curve {
        CurveGeometry::Line { origin, direction } => {
            let squared =
                direction.x * direction.x + direction.y * direction.y + direction.z * direction.z;
            if !squared.is_finite() || squared <= 0.0 {
                return None;
            }
            let range = endpoints.map(|p| {
                ((p.x - origin.x) * direction.x
                    + (p.y - origin.y) * direction.y
                    + (p.z - origin.z) * direction.z)
                    / squared
            });
            (range, GeometryIntervalMethod::LineProjection)
        }
        CurveGeometry::Nurbs(nurbs) => {
            let degree = usize::try_from(nurbs.degree).ok()?;
            let count = nurbs.control_points.len();
            if nurbs.periodic
                || degree == 0
                || degree > 32
                || count <= degree
                || nurbs.knots.len() != count.checked_add(degree)?.checked_add(1)?
                || nurbs.knots.iter().any(|v| !v.is_finite())
                || nurbs.knots.windows(2).any(|p| p[0] > p[1])
                || nurbs
                    .control_points
                    .iter()
                    .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
                || nurbs.weights.as_ref().is_some_and(|w| {
                    w.len() != count || w.iter().any(|v| !v.is_finite() || *v <= 0.0)
                })
            {
                return None;
            }
            let range = [nurbs.knots[degree], nurbs.knots[count]];
            if range[0] >= range[1]
                || nurbs.knots[..=degree].iter().any(|k| *k != range[0])
                || nurbs.knots[count..].iter().any(|k| *k != range[1])
            {
                return None;
            }
            (range, GeometryIntervalMethod::NurbsSupportEndpoints)
        }
        _ => return None,
    };
    if range.iter().any(|v| !v.is_finite()) || range[0] == range[1] {
        return None;
    }
    let evaluated = [
        cadmpeg_ir::eval::curve_point(curve, range[0])?,
        cadmpeg_ir::eval::curve_point(curve, range[1])?,
    ];
    if evaluated
        .iter()
        .any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite())
    {
        return None;
    }
    let forward = distance(evaluated[0], endpoints[0]).max(distance(evaluated[1], endpoints[1]));
    let backward = distance(evaluated[0], endpoints[1]).max(distance(evaluated[1], endpoints[0]));
    let forward_matches = forward.is_finite() && forward <= TOLERANCE_MM;
    let backward_matches = backward.is_finite() && backward <= TOLERANCE_MM;
    if forward_matches == backward_matches {
        return None;
    }
    if backward_matches {
        range.swap(0, 1);
    }
    Some(GeometryDerivedInterval {
        parameter_range: range,
        method,
        tolerance_mm: TOLERANCE_MM,
        max_endpoint_error_mm: if forward_matches { forward } else { backward },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadmpeg_ir::{geometry::NurbsCurve, math::Vector3};

    #[test]
    fn line_projection_keeps_signed_parameters_and_rejects_off_curve_vertices() {
        let line = CurveGeometry::Line {
            origin: Point3::new(10.0, 0.0, 0.0),
            direction: Vector3::new(2.0, 0.0, 0.0),
        };
        let ends = [Point3::new(14.0, 0.0, 0.0), Point3::new(6.0, 0.0, 0.0)];
        assert_eq!(
            derive(&line, ends).map(|v| v.parameter_range),
            Some([2.0, -2.0])
        );
        assert!(derive(&line, [ends[0], Point3::new(6.0, 0.001, 0.0)]).is_none());
        assert!(derive(&line, [ends[0], ends[0]]).is_none());
    }

    #[test]
    fn nurbs_support_requires_clamping_unique_endpoint_order_and_complete_support() {
        let nurbs = NurbsCurve {
            degree: 2,
            knots: vec![2.0, 2.0, 2.0, 5.0, 5.0, 5.0],
            control_points: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
                Point3::new(2.0, 0.0, 0.0),
            ],
            weights: None,
            periodic: false,
        };
        let ends = [nurbs.control_points[2], nurbs.control_points[0]];
        let curve = CurveGeometry::Nurbs(nurbs.clone());
        assert_eq!(
            derive(&curve, ends).map(|v| v.parameter_range),
            Some([5.0, 2.0])
        );
        assert!(derive(&curve, [Point3::new(1.0, 0.5, 0.0), ends[1]]).is_none());
        for case in 0..5 {
            let mut invalid = nurbs.clone();
            match case {
                0 => invalid.periodic = true,
                1 => invalid.knots[0] = 1.0,
                2 => invalid.weights = Some(vec![1.0, -1.0, 1.0]),
                3 => invalid.knots.clear(),
                _ => invalid.control_points[1].x = f64::NAN,
            }
            assert!(derive(&CurveGeometry::Nurbs(invalid), ends).is_none());
        }
    }

    #[test]
    fn conic_intervals_keep_positive_native_direction_across_the_branch_cut()
    -> Result<(), &'static str> {
        use std::f64::consts::{FRAC_PI_2, PI, TAU};
        let circle = CurveGeometry::Circle {
            center: Point3::new(10., 20., 30.),
            axis: Vector3::new(0., 0., 1.),
            ref_direction: Vector3::new(1., 0., 0.),
            radius: 2.,
        };
        let point = |t: f64| Point3::new(10.0 + 2.0 * t.cos(), 20.0 + 2.0 * t.sin(), 30.0);
        let crossing = conic_interval(&circle, [point(3. * FRAC_PI_2), point(FRAC_PI_2)], false)
            .ok_or("valid conic interval expected")?;
        assert!((crossing.parameter_range[1] - crossing.parameter_range[0] - PI).abs() < 1e-12);
        let major_arc = conic_interval(&circle, [point(0.), point(3. * FRAC_PI_2)], false)
            .ok_or("valid conic interval expected")?;
        assert!((major_arc.parameter_range[1] - 3. * FRAC_PI_2).abs() < 1e-12);
        assert!(conic_interval(&circle, [point(0.), point(0.)], false).is_none());
        let seam = conic_interval(&circle, [point(PI), point(PI)], true)
            .ok_or("valid conic interval expected")?;
        assert_eq!(seam.method, GeometryIntervalMethod::ClosedCircleSeam);
        assert!((seam.parameter_range[1] - seam.parameter_range[0] - TAU).abs() < 1e-12);
        assert!(conic_interval(&circle, [point(0.), Point3::new(10., 22., 31.)], false).is_none());
        let ellipse = CurveGeometry::Ellipse {
            center: Point3::new(0., 0., 0.),
            axis: Vector3::new(0., 0., -1.),
            major_direction: Vector3::new(1., 0., 0.),
            major_radius: 3.,
            minor_radius: 1.,
        };
        let range = conic_interval(
            &ellipse,
            [Point3::new(3., 0., 0.), Point3::new(0., -1., 0.)],
            false,
        )
        .ok_or("valid conic interval expected")?;
        assert!((range.parameter_range[1] - FRAC_PI_2).abs() < 1e-12);
        assert!(conic_interval(&ellipse, [Point3::new(3., 0., 0.); 2], true).is_none());
        Ok(())
    }

    #[test]
    fn decoded_ranges_are_never_overwritten_or_reclassified() {
        let mut ir = cadmpeg_ir::examples::unit_cube();
        let edge = &mut ir.model.edges[0];
        edge.param_range = Some([12.0, 13.0]);
        assert!(
            derive_for_edge(
                &ir.model.edges[0],
                &Sources::new(&ir, &cadmpeg_ir::Annotations::default())
            )
            .is_none()
        );
        assert_eq!(ir.model.edges[0].param_range, Some([12.0, 13.0]));
    }
}
