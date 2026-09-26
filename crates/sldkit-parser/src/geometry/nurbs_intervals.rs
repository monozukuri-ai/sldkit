//! Partial intervals with a global, conservative uniqueness certificate.
//! Positive weights preserve the sign changes of a projected control polygon.
//! A strictly monotone coordinate therefore admits at most one inverse point.
use cadmpeg_ir::{geometry::NurbsCurve, math::Point3};

const TOL: f64 = 1e-7;
fn coordinate(p: Point3, axis: usize) -> f64 {
    [p.x, p.y, p.z][axis]
}
fn distance(a: Point3, b: Point3) -> f64 {
    (a.x - b.x).hypot(a.y - b.y).hypot(a.z - b.z)
}

// Knot multiplicities are exact structural facts, not approximate geometry.
#[allow(clippy::float_cmp, clippy::too_many_lines)]
pub(super) fn interval(curve: &NurbsCurve, ends: [Point3; 2]) -> Option<([f64; 2], f64)> {
    let degree = usize::try_from(curve.degree).ok()?;
    let count = curve.control_points.len();
    if curve.periodic
        || !(1..=16).contains(&degree)
        || count <= degree
        || count > 1024
        || curve.knots.len() != count + degree + 1
        || curve.knots.iter().any(|v| !v.is_finite())
        || curve.knots.windows(2).any(|v| v[0] > v[1])
        || curve
            .control_points
            .iter()
            .chain(ends.iter())
            .any(|p| [p.x, p.y, p.z].iter().any(|v| !v.is_finite()))
        || curve.weights.as_ref().is_some_and(|w| {
            w.len() != count
                || w.iter()
                    .any(|v| !v.is_finite() || !(1e-12..=1e12).contains(v))
        })
        || distance(ends[0], ends[1]) <= 2.0 * TOL
    {
        return None;
    }
    let domain = [curve.knots[degree], curve.knots[count]];
    if domain[0] >= domain[1]
        || !(domain[1] - domain[0]).is_finite()
        || curve.knots[..=degree].iter().any(|v| *v != domain[0])
        || curve.knots[count..].iter().any(|v| *v != domain[1])
    {
        return None;
    }
    // An interior multiplicity of degree+1 could create a discontinuous jump.
    if curve.knots[degree + 1..count]
        .windows(degree + 1)
        .any(|w| w.iter().all(|v| *v == w[0]))
    {
        return None;
    }
    let scale = curve
        .control_points
        .iter()
        .chain(ends.iter())
        .flat_map(|p| [p.x.abs(), p.y.abs(), p.z.abs()])
        .fold(1.0_f64, f64::max);
    let roundoff = 64.0 * f64::EPSILON * scale;
    if roundoff > TOL / 8.0 {
        return None;
    }
    let (axis, sign) = (0..3).find_map(|axis| {
        let sign = (coordinate(curve.control_points[count - 1], axis)
            - coordinate(curve.control_points[0], axis))
        .signum();
        curve
            .control_points
            .windows(2)
            .all(|p| (coordinate(p[1], axis) - coordinate(p[0], axis)) * sign > roundoff)
            .then_some((axis, sign))
    })?;
    let evaluate = |t| {
        cadmpeg_ir::eval::nurbs_curve_point(
            curve.degree,
            &curve.knots,
            &curve.control_points,
            curve.weights.as_deref(),
            t,
        )
    };
    let mut parameters = [0.0; 2];
    let mut error = 0.0_f64;
    for (i, target) in ends.into_iter().enumerate() {
        let value = coordinate(target, axis) * sign;
        let low = coordinate(curve.control_points[0], axis) * sign;
        let high = coordinate(curve.control_points[count - 1], axis) * sign;
        if value < low - TOL || value > high + TOL {
            return None;
        }
        let mut range = domain;
        for _ in 0..100 {
            let middle = range[0] + (range[1] - range[0]) / 2.0;
            if middle == range[0] || middle == range[1] {
                break;
            }
            let point = evaluate(middle)?;
            if coordinate(point, axis) * sign < value {
                range[0] = middle;
            } else {
                range[1] = middle;
            }
        }
        let candidates = [range[0], range[1]];
        let mut best = None;
        for parameter in candidates {
            let residual = distance(evaluate(parameter)?, target);
            if residual.is_finite() && best.is_none_or(|(_, e)| residual < e) {
                best = Some((parameter, residual));
            }
        }
        let (parameter, residual) = best?;
        if residual > TOL {
            return None;
        }
        parameters[i] = parameter;
        error = error.max(residual);
    }
    (parameters[0] != parameters[1]).then_some((parameters, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> NurbsCurve {
        NurbsCurve {
            degree: 2,
            knots: vec![2., 2., 2., 4., 6., 6., 6.],
            control_points: vec![
                Point3::new(0., 0., 0.),
                Point3::new(1., 2., 0.),
                Point3::new(3., -1., 0.),
                Point3::new(5., 1., 0.),
            ],
            weights: Some(vec![1., 0.5, 2., 1.]),
            periodic: false,
        }
    }
    #[test]
    fn rational_multispan_partial_interval_preserves_endpoint_order() -> Result<(), &'static str> {
        let c = fixture();
        let point = |t| {
            cadmpeg_ir::eval::nurbs_curve_point(
                c.degree,
                &c.knots,
                &c.control_points,
                c.weights.as_deref(),
                t,
            )
            .ok_or("valid curve")
        };
        for expected in [[2.4, 5.3], [5.3, 2.4], [4., 5.3], [2., 4.]] {
            // Independently evaluated with OCP 7.9.3.1 Geom_BSplineCurve.Value.
            let oracle = |parameter: f64| match parameter {
                2.4 => Point3::new(0.341_176_470_588_235_2, 0.352_941_176_470_588_15, 0.),
                5.3 => Point3::new(3.527_555_742_532_603_4, -0.369_373_159_444_678_3, 0.),
                4.0 => Point3::new(2.6, -0.4, 0.),
                _ => Point3::new(0., 0., 0.),
            };
            let (actual, error) = interval(&c, [oracle(expected[0]), oracle(expected[1])])
                .ok_or("unique partial interval")?;
            assert!((actual[0] - expected[0]).abs() < 1e-10);
            assert!((actual[1] - expected[1]).abs() < 1e-10);
            assert!(error < TOL);
        }
        let mut off = point(3.)?;
        off.z = 1e-4;
        assert!(interval(&c, [off, point(5.)?]).is_none());
        Ok(())
    }
    #[test]
    fn ambiguous_periodic_discontinuous_or_oversized_support_is_withheld() {
        for case in 0..6 {
            let mut c = fixture();
            match case {
                0 => {
                    c.control_points[2].x = -1.;
                }
                1 => c.periodic = true,
                2 => c.weights = Some(vec![1., -1., 1., 1.]),
                3 => c.control_points[0].x = f64::NAN,
                4 => {
                    c.degree = 1;
                    c.knots = vec![0., 0., 0.5, 0.5, 1., 1.];
                }
                _ => c.control_points[0].x = -1e12,
            }
            assert!(interval(&c, [Point3::new(0.5, 0., 0.), Point3::new(4., 0., 0.)]).is_none());
        }
    }
}
