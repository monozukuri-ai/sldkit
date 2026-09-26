//! Bounded planar loop classification using exact line/circular-arc geometry.
//! No loop-list-order assumptions, fitted polygons, or periodic-face roles.

use super::intervals::{self, Sources};
use cadmpeg_ir::{
    Annotations, CadIr,
    geometry::{PcurveGeometry, SurfaceGeometry},
    topology::{LoopBoundaryRole, Sense},
};
use sldkit_core::{GeometryDerivedLoopRole, GeometryLoopRoleMethod};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::TAU,
};

const TOL: f64 = 1e-7;
type P = [f64; 2];
fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn scale(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: P, b: P) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn dist(a: P, b: P) -> f64 {
    let d = sub(a, b);
    d[0].hypot(d[1])
}

#[derive(Clone, Debug)]
enum Piece {
    Line(P, P),
    Arc {
        center: P,
        radius: f64,
        start: f64,
        sweep: f64,
    },
}
impl Piece {
    fn point(&self, fraction: f64) -> P {
        match *self {
            Self::Line(a, b) => add(a, scale(sub(b, a), fraction)),
            Self::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let angle = start + sweep * fraction;
                add(center, [radius * angle.cos(), radius * angle.sin()])
            }
        }
    }
    fn contains(&self, point: P) -> bool {
        match *self {
            Self::Line(a, b) => {
                let d = sub(b, a);
                let length = dist(a, b);
                let t = dot(sub(point, a), d) / (length * length);
                t >= -TOL / length
                    && t <= 1.0 + TOL / length
                    && cross(sub(point, a), d).abs() / length <= TOL
            }
            Self::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let d = sub(point, center);
                let phase = ((d[1].atan2(d[0]) - start) * sweep.signum()).rem_euclid(TAU);
                (dist(point, center) - radius).abs() <= TOL
                    && (phase <= sweep.abs() + TOL / radius || TAU - phase <= TOL / radius)
            }
        }
    }
    fn area(&self, origin: P) -> f64 {
        match *self {
            Self::Line(a, b) => cross(sub(a, origin), sub(b, origin)) / 2.0,
            Self::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let c = sub(center, origin);
                let end = start + sweep;
                (radius * c[0] * (end.sin() - start.sin())
                    + radius * c[1] * (start.cos() - end.cos())
                    + radius * radius * sweep)
                    / 2.0
            }
        }
    }
}

fn angular_spans(piece: &Piece) -> Vec<[f64; 2]> {
    let Piece::Arc { start, sweep, .. } = *piece else {
        return vec![];
    };
    if sweep.abs() >= TAU {
        return vec![[0.0, TAU]];
    }
    let low = (start + sweep.min(0.0)).rem_euclid(TAU);
    let high = low + sweep.abs();
    if high <= TAU {
        vec![[low, high]]
    } else {
        vec![[low, TAU], [0.0, high - TAU]]
    }
}

/// None means overlapping or near-coincident supports are ambiguous.
// Exact equality distinguishes identical supports from nearly coincident ones.
#[allow(clippy::float_cmp)]
fn intersections(a: &Piece, b: &Piece) -> Option<Vec<P>> {
    let candidates = match (a, b) {
        (Piece::Line(a0, a1), Piece::Line(b0, b1)) => {
            let ad = sub(*a1, *a0);
            let bd = sub(*b1, *b0);
            let det = cross(ad, bd);
            if det.abs() <= 1e-12 * dist(*a0, *a1) * dist(*b0, *b1) {
                let shared = [*a0, *a1, *b0, *b1]
                    .into_iter()
                    .filter(|p| a.contains(*p) && b.contains(*p))
                    .collect::<Vec<_>>();
                if shared
                    .iter()
                    .any(|p| shared.iter().any(|q| dist(*p, *q) > TOL))
                {
                    return None;
                }
                shared
            } else {
                vec![add(*a0, scale(ad, cross(sub(*b0, *a0), bd) / det))]
            }
        }
        (Piece::Arc { .. }, Piece::Line(..)) => return intersections(b, a),
        (Piece::Line(start, end), Piece::Arc { center, radius, .. }) => {
            let d = sub(*end, *start);
            let squared = dot(d, d);
            let closest = add(*start, scale(d, dot(sub(*center, *start), d) / squared));
            let h2 = radius * radius - dot(sub(closest, *center), sub(closest, *center));
            if h2 < -TOL * (2.0 * radius + TOL) {
                vec![]
            } else {
                let offset = scale(d, (h2.max(0.0) / squared).sqrt());
                vec![add(closest, offset), sub(closest, offset)]
            }
        }
        (
            Piece::Arc {
                center: ca,
                radius: ra,
                ..
            },
            Piece::Arc {
                center: cb,
                radius: rb,
                ..
            },
        ) => {
            let d = dist(*ca, *cb);
            if d <= TOL {
                if (ra - rb).abs() <= TOL {
                    if ca != cb || ra != rb {
                        return None;
                    }
                    for left in angular_spans(a) {
                        for right in angular_spans(b) {
                            if left[1].min(right[1]) - left[0].max(right[0]) > TOL / ra {
                                return None;
                            }
                        }
                    }
                    vec![a.point(0.0), a.point(1.0), b.point(0.0), b.point(1.0)]
                } else {
                    vec![]
                }
            } else if d > ra + rb + TOL || d < (ra - rb).abs() - TOL {
                vec![]
            } else {
                let x = (ra * ra - rb * rb + d * d) / (2.0 * d);
                let h2 = ra * ra - x * x;
                if h2 < -TOL * (2.0 * ra + TOL) {
                    return None;
                }
                let axis = scale(sub(*cb, *ca), 1.0 / d);
                let center = add(*ca, scale(axis, x));
                let offset = scale([-axis[1], axis[0]], h2.max(0.0).sqrt());
                vec![add(center, offset), sub(center, offset)]
            }
        }
    };
    Some(
        candidates
            .into_iter()
            .filter(|p| a.contains(*p) && b.contains(*p))
            .collect(),
    )
}

fn simple(ring: &[Piece]) -> bool {
    if ring.is_empty() || ring.len() > 512 {
        return false;
    }
    for (i, a) in ring.iter().enumerate() {
        if dist(a.point(1.0), ring[(i + 1) % ring.len()].point(0.0)) > TOL {
            return false;
        }
        for (j, b) in ring.iter().enumerate().skip(i + 1) {
            let Some(points) = intersections(a, b) else {
                return false;
            };
            for p in points {
                let forward =
                    j == i + 1 && dist(p, a.point(1.0)) <= TOL && dist(p, b.point(0.0)) <= TOL;
                let closure = i == 0
                    && j + 1 == ring.len()
                    && dist(p, a.point(0.0)) <= TOL
                    && dist(p, b.point(1.0)) <= TOL;
                if !forward && !closure {
                    return false;
                }
            }
        }
    }
    true
}

/// Exact horizontal-ray crossings, with a half-open convention at shared ends.
fn inside(p: P, ring: &[Piece]) -> Option<bool> {
    let mut winding = 0_i32;
    for piece in ring {
        if piece.contains(p) {
            return None;
        }
        match *piece {
            Piece::Line(a, b) => {
                if (a[1] <= p[1] && b[1] > p[1]) || (b[1] <= p[1] && a[1] > p[1]) {
                    let x = a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
                    if x > p[0] {
                        winding += if b[1] > a[1] { 1 } else { -1 };
                    }
                }
            }
            Piece::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let h = (p[1] - center[1]) / radius;
                if h.abs() >= 1.0 {
                    continue;
                }
                let angle = h.asin();
                for theta in [angle, std::f64::consts::PI - angle] {
                    let q = [center[0] + radius * theta.cos(), p[1]];
                    if q[0] <= p[0] || !piece.contains(q) {
                        continue;
                    }
                    let dy = theta.cos() * sweep;
                    let phase = ((theta - start) * sweep.signum()).rem_euclid(TAU);
                    if sweep.abs() < TAU - 1e-10 {
                        let at_start = phase <= 1e-10 || TAU - phase <= 1e-10;
                        let at_end = (phase - sweep.abs()).abs() <= 1e-10;
                        if (at_start && dy < 0.0) || (at_end && dy > 0.0) {
                            continue;
                        }
                    }
                    winding += if dy > 0.0 { 1 } else { -1 };
                }
            }
        }
    }
    match winding.abs() {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn classify(rings: &[Vec<Piece>], face_sign: f64) -> Option<Vec<f64>> {
    if rings.is_empty() || rings.len() > 128 || rings.iter().any(|r| !simple(r)) {
        return None;
    }
    for (i, a) in rings.iter().enumerate() {
        for b in rings.iter().skip(i + 1) {
            for pa in a {
                for pb in b {
                    if !intersections(pa, pb)?.is_empty() {
                        return None;
                    }
                }
            }
        }
    }
    let areas = rings
        .iter()
        .map(|r| r.iter().map(|p| p.area(r[0].point(0.0))).sum::<f64>() * face_sign)
        .collect::<Vec<_>>();
    if areas.iter().any(|a| !a.is_finite() || a.abs() <= TOL * TOL) {
        return None;
    }
    let outer = areas
        .iter()
        .enumerate()
        .filter(|(_, a)| **a > 0.0)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let [outer] = outer.as_slice() else {
        return None;
    };
    for (i, ring) in rings.iter().enumerate().filter(|(i, _)| i != outer) {
        let p = ring[0].point(0.0);
        if !inside(p, &rings[*outer])? {
            return None;
        }
        for (_, other) in rings
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i && j != outer)
        {
            if inside(p, other)? {
                return None;
            }
        }
    }
    Some(areas)
}

#[allow(clippy::too_many_lines)]
pub(super) fn derive(
    ir: &CadIr,
    annotations: &Annotations,
    sources: &Sources<'_>,
) -> BTreeMap<String, GeometryDerivedLoopRole> {
    let loops = ir
        .model
        .loops
        .iter()
        .map(|v| (&v.id, v))
        .collect::<BTreeMap<_, _>>();
    let coedges = ir
        .model
        .coedges
        .iter()
        .map(|v| (&v.id, v))
        .collect::<BTreeMap<_, _>>();
    let edges = ir
        .model
        .edges
        .iter()
        .map(|v| (&v.id, v))
        .collect::<BTreeMap<_, _>>();
    let pcurves = ir
        .model
        .pcurves
        .iter()
        .map(|v| (&v.id, v))
        .collect::<BTreeMap<_, _>>();
    let surfaces = ir
        .model
        .surfaces
        .iter()
        .map(|v| (&v.id, v))
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::new();
    for face in &ir.model.faces {
        let Some(surface) = surfaces.get(&face.surface) else {
            continue;
        };
        if !matches!(surface.geometry, SurfaceGeometry::Plane { .. })
            || !sources.native_entity(face.id.as_str())
        {
            continue;
        }
        let rings = (|| {
            let mut rings = Vec::new();
            let mut seen = BTreeSet::new();
            if face.loops.len() > 128 {
                return None;
            }
            for id in &face.loops {
                let lp = *loops.get(id)?;
                if lp.face != face.id
                    || lp.boundary_role != LoopBoundaryRole::Unspecified
                    || !lp.vertex_uses.is_empty()
                    || lp.coedges.is_empty()
                    || lp.coedges.len() > 512
                {
                    return None;
                }
                let mut ring = Vec::new();
                for (i, id) in lp.coedges.iter().enumerate() {
                    if !seen.insert(id) || seen.len() > 512 {
                        return None;
                    }
                    let c = *coedges.get(id)?;
                    if c.owner_loop != lp.id
                        || c.next != lp.coedges[(i + 1) % lp.coedges.len()]
                        || c.previous != lp.coedges[(i + lp.coedges.len() - 1) % lp.coedges.len()]
                        || c.use_curve.is_some()
                        || c.use_curve_parameter_range.is_some()
                    {
                        return None;
                    }
                    let edge = *edges.get(&c.edge)?;
                    let mut range = edge.param_range.or_else(|| {
                        intervals::derive_for_edge(edge, sources).map(|v| v.parameter_range)
                    })?;
                    if range.iter().any(|v| !v.is_finite()) {
                        return None;
                    }
                    if c.sense == Sense::Reversed {
                        range.swap(0, 1);
                    }
                    let [use_] = c.pcurves.as_slice() else {
                        return None;
                    };
                    let pcurve = *pcurves.get(&use_.pcurve)?;
                    if annotations
                        .provenance
                        .get(pcurve.id.as_str())?
                        .tag
                        .as_deref()
                        != Some("derived_planar_pcurve")
                    {
                        return None;
                    }
                    let uv = |t| {
                        let p = cadmpeg_ir::eval::pcurve_uv(&pcurve.geometry, t)?;
                        (p.u.is_finite() && p.v.is_finite()).then_some([p.u, p.v])
                    };
                    let piece = match pcurve.geometry {
                        PcurveGeometry::Line { .. } => {
                            let a = uv(range[0])?;
                            let b = uv(range[1])?;
                            if dist(a, b) <= 2.0 * TOL {
                                return None;
                            }
                            Piece::Line(a, b)
                        }
                        PcurveGeometry::Circle {
                            center,
                            x_axis,
                            y_axis,
                            radius,
                        } => {
                            let determinant = x_axis.u * y_axis.v - x_axis.v * y_axis.u;
                            if !radius.is_finite()
                                || radius <= TOL
                                || [center.u, center.v, x_axis.u, x_axis.v, y_axis.u, y_axis.v]
                                    .iter()
                                    .any(|v| !v.is_finite())
                                || (x_axis.u.hypot(x_axis.v) - 1.0).abs() > 1e-9
                                || (y_axis.u.hypot(y_axis.v) - 1.0).abs() > 1e-9
                                || (x_axis.u * y_axis.u + x_axis.v * y_axis.v).abs() > 1e-9
                                || (determinant.abs() - 1.0).abs() > 1e-9
                            {
                                return None;
                            }
                            let sweep = determinant * (range[1] - range[0]);
                            if !sweep.is_finite()
                                || sweep.abs() > TAU + 1e-10
                                || sweep.abs() * radius <= 2.0 * TOL
                            {
                                return None;
                            }
                            Piece::Arc {
                                center: [center.u, center.v],
                                radius,
                                start: x_axis.v.atan2(x_axis.u) + determinant * range[0],
                                sweep,
                            }
                        }
                        _ => return None,
                    };
                    ring.push(piece);
                }
                rings.push(ring);
            }
            Some(rings)
        })();
        let Some(rings) = rings else {
            continue;
        };
        let Some(areas) = classify(
            &rings,
            if face.sense == Sense::Forward {
                1.0
            } else {
                -1.0
            },
        ) else {
            continue;
        };
        for (id, area) in face.loops.iter().zip(areas) {
            result.insert(
                id.0.clone(),
                GeometryDerivedLoopRole {
                    role: if area > 0.0 { "outer" } else { "inner" }.into(),
                    method: GeometryLoopRoleMethod::PlanarAnalyticWinding,
                    signed_area_mm2: area,
                    tolerance_mm: TOL,
                },
            );
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rectangle(low: P, high: P) -> Vec<Piece> {
        let points = [low, [high[0], low[1]], high, [low[0], high[1]]];
        (0..4)
            .map(|i| Piece::Line(points[i], points[(i + 1) % 4]))
            .collect()
    }
    fn circle(center: P, radius: f64, sweep: f64) -> Vec<Piece> {
        vec![Piece::Arc {
            center,
            radius,
            start: 0.37,
            sweep,
        }]
    }
    #[test]
    fn outer_and_holes_ignore_list_order_and_follow_face_sense() -> Result<(), &'static str> {
        let outer = rectangle([-3., -3.], [3., 3.]);
        let hole = circle([0., 0.], 1., -TAU);
        let areas = classify(&[hole, outer], 1.).ok_or("valid analytic test fixture expected")?;
        assert!(areas[0] < 0. && areas[1] > 0.);
        assert!((areas[0] + std::f64::consts::PI).abs() < 1e-12);
        let reversed = rectangle([-3., -3.], [3., 3.])
            .into_iter()
            .rev()
            .map(|p| {
                let Piece::Line(a, b) = p else { unreachable!() };
                Piece::Line(b, a)
            })
            .collect();
        assert!(classify(&[reversed, circle([0., 0.], 1., TAU)], -1.).is_some());
        Ok(())
    }
    #[test]
    fn crossing_touching_nested_and_outside_holes_are_withheld() {
        for holes in [
            vec![circle([4., 0.], 1., -TAU)],
            vec![circle([2., 0.], 1., -TAU)],
            vec![circle([0., 0.], 2., -TAU), circle([0., 0.], 1., -TAU)],
            vec![circle([-0.5, 0.], 1., -TAU), circle([0.5, 0.], 1., -TAU)],
        ] {
            let mut rings = vec![rectangle([-3., -3.], [3., 3.])];
            rings.extend(holes);
            assert!(classify(&rings, 1.).is_none());
        }
        let crossing = vec![
            Piece::Line([0., 0.], [2., 2.]),
            Piece::Line([2., 2.], [0., 2.]),
            Piece::Line([0., 2.], [2., 0.]),
            Piece::Line([2., 0.], [0., 0.]),
        ];
        assert!(!simple(&crossing));
        let mut open = rectangle([0., 0.], [2., 2.]);
        open.pop();
        assert!(!simple(&open));
    }
    #[test]
    fn disjoint_arcs_on_one_circle_are_not_mistaken_for_overlap() -> Result<(), &'static str> {
        let arc = |start, sweep| Piece::Arc {
            center: [0., 0.],
            radius: 2.,
            start,
            sweep,
        };
        assert!(
            intersections(&arc(0., 0.5), &arc(1., 0.5))
                .ok_or("valid analytic test fixture expected")?
                .is_empty()
        );
        assert!(intersections(&arc(0., 1.), &arc(0.5, 1.)).is_none());
        assert!(
            !intersections(&arc(0., 0.5), &arc(0.5, 0.5))
                .ok_or("valid analytic test fixture expected")?
                .is_empty()
        );
        assert!(intersections(&arc(5.5, 1.), &arc(0., 0.5)).is_none());
        Ok(())
    }
}
