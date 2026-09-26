//! Bounded endpoint-derived intervals; never overwrite native trim metadata.
use std::collections::BTreeMap;

use cadmpeg_ir::{CadIr, geometry::CurveGeometry, math::Point3, topology::Edge};
use sldkit_core::{GeometryDerivedInterval, GeometryIntervalMethod};

const TOLERANCE_MM: f64 = 1.0e-7;

pub(super) struct Sources<'a> {
    curves: BTreeMap<&'a str, &'a CurveGeometry>,
    vertices: BTreeMap<&'a str, Point3>,
}

impl<'a> Sources<'a> {
    pub(super) fn new(ir: &'a CadIr) -> Self {
        let points: BTreeMap<_, _> = ir
            .model
            .points
            .iter()
            .map(|point| (point.id.as_str(), point.position))
            .collect();
        Self {
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
    derive(curve, endpoints)
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
    fn decoded_ranges_are_never_overwritten_or_reclassified() {
        let mut ir = cadmpeg_ir::examples::unit_cube();
        let edge = &mut ir.model.edges[0];
        edge.param_range = Some([12.0, 13.0]);
        assert!(derive_for_edge(&ir.model.edges[0], &Sources::new(&ir)).is_none());
        assert_eq!(ir.model.edges[0].param_range, Some([12.0, 13.0]));
    }
}
