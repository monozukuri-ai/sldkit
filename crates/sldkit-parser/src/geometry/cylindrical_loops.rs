//! Simple cylinder charts with disjoint holes and an optional cut seam pair.
//! The chart is unwrapped continuously; raw atan2 jumps are not trim boundaries.
use super::intervals::{self, Sources};
use cadmpeg_ir::{
    Annotations, CadIr,
    geometry::{PcurveGeometry, SurfaceGeometry},
    topology::Sense,
};
use sldkit_core::{GeometryDerivedLoopRole, GeometryLoopRoleMethod};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::TAU,
};
const TOL: f64 = 1e-7;
type P = [f64; 2];

#[derive(Clone, Debug)]
struct Segment {
    start: P,
    end: P,
    graph: Option<[f64; 3]>,
}
impl Segment {
    fn value(&self, u: f64) -> f64 {
        let [a, b, c] = self.graph.unwrap_or([f64::NAN; 3]);
        a + b * u.cos() + c * u.sin()
    }
    fn bounds(&self) -> [f64; 2] {
        [
            self.start[0].min(self.end[0]),
            self.start[0].max(self.end[0]),
        ]
    }
    fn has(&self, p: P, utol: f64) -> bool {
        let [lo, hi] = self.bounds();
        if p[0] < lo - utol || p[0] > hi + utol {
            return false;
        }
        if self.graph.is_some() {
            (self.value(p[0]) - p[1]).abs() <= TOL
        } else {
            p[1] >= self.start[1].min(self.end[1]) - TOL
                && p[1] <= self.start[1].max(self.end[1]) + TOL
        }
    }
    fn integral(&self, origin: f64) -> f64 {
        let Some([constant, cosine, sine]) = self.graph else {
            return 0.0;
        };
        let [u, v] = [self.start[0], self.end[0]];
        -(constant - origin) * (v - u) - cosine * (v.sin() - u.sin()) + sine * (v.cos() - u.cos())
    }
}

fn near(a: P, b: P, utol: f64) -> bool {
    (a[0] - b[0]).abs() <= utol && (a[1] - b[1]).abs() <= TOL
}

#[allow(clippy::many_single_char_names)]
fn crossings(a: &Segment, b: &Segment, utol: f64) -> Option<Vec<P>> {
    let [al, ah] = a.bounds();
    let [bl, bh] = b.bounds();
    let low = al.max(bl);
    let high = ah.min(bh);
    if low > high + utol {
        return Some(vec![]);
    }
    let candidates = match (a.graph, b.graph) {
        (None, None) => {
            let lo = a.start[1].min(a.end[1]).max(b.start[1].min(b.end[1]));
            let hi = a.start[1].max(a.end[1]).min(b.start[1].max(b.end[1]));
            if hi - lo > TOL {
                return None;
            }
            vec![[a.start[0], lo]]
        }
        (None, Some(_)) => vec![[a.start[0], b.value(a.start[0])]],
        (Some(_), None) => vec![[b.start[0], a.value(b.start[0])]],
        (Some(x), Some(y)) => {
            let [c, p, q] = [x[0] - y[0], x[1] - y[1], x[2] - y[2]];
            let radius = p.hypot(q);
            if radius <= TOL {
                if c.abs() <= 2.0 * TOL && high - low > utol {
                    return None;
                }
                vec![[low, a.value(low)], [high, a.value(high)]]
            } else {
                let ratio = -c / radius;
                if ratio.abs() > 1.0 + TOL / radius {
                    return Some(vec![]);
                }
                let phase = q.atan2(p);
                let angle = ratio.clamp(-1.0, 1.0).acos();
                let mut points = Vec::new();
                for root in [phase + angle, phase - angle] {
                    let base = root + ((low - root) / TAU).floor() * TAU;
                    for u in [base, base + TAU, base + 2.0 * TAU] {
                        if u >= low - utol && u <= high + utol {
                            points.push([u, a.value(u)]);
                        }
                    }
                }
                points
            }
        }
    };
    Some(
        candidates
            .into_iter()
            .filter(|p| a.has(*p, utol) && b.has(*p, utol))
            .collect(),
    )
}

fn chart_area(ring: &mut [Segment], radius: f64, sign: f64, cut_seam: bool) -> Option<f64> {
    if ring.len() < 4 || ring.len() > 128 || !radius.is_finite() || radius <= TOL {
        return None;
    }
    let utol = TOL / radius;
    for i in 1..ring.len() {
        let shift = ((ring[i - 1].end[0] - ring[i].start[0]) / TAU).round() * TAU;
        ring[i].start[0] += shift;
        ring[i].end[0] += shift;
        if !near(ring[i - 1].end, ring[i].start, utol) {
            return None;
        }
    }
    if !near(ring[ring.len() - 1].end, ring[0].start, utol) {
        return None;
    }
    let low = ring
        .iter()
        .map(|s| s.bounds()[0])
        .fold(f64::INFINITY, f64::min);
    let high = ring
        .iter()
        .map(|s| s.bounds()[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let width = high - low;
    if width > TAU + utol || (!cut_seam && width >= TAU - utol) {
        return None;
    }
    // A full-period chart is admitted only when its two extreme axial sides
    // are the same explicit derived seam traversed in opposite directions.
    if cut_seam
        && ((width - TAU).abs() > utol
            || ring.len() != 4
            || ring.iter().filter(|s| s.graph.is_none()).count() != 2)
    {
        return None;
    }
    for (i, a) in ring.iter().enumerate() {
        for (j, b) in ring.iter().enumerate().skip(i + 1) {
            for p in crossings(a, b, utol)? {
                if !(j == i + 1 && near(p, a.end, utol) && near(p, b.start, utol))
                    && !(i == 0
                        && j + 1 == ring.len()
                        && near(p, a.start, utol)
                        && near(p, b.end, utol))
                {
                    return None;
                }
            }
        }
    }
    let area = ring
        .iter()
        .map(|s| s.integral(ring[0].start[1]))
        .sum::<f64>()
        * radius
        * sign;
    (area.is_finite() && area.abs() > TOL * TOL).then_some(area)
}

struct Chart {
    ring: Vec<Segment>,
    cut_seam: bool,
}

fn bounds(ring: &[Segment]) -> [f64; 2] {
    ring.iter()
        .fold([f64::INFINITY, f64::NEG_INFINITY], |b, s| {
            [b[0].min(s.bounds()[0]), b[1].max(s.bounds()[1])]
        })
}

/// Vertical ray parity on analytic graphs. Near a vertex or the boundary,
/// withhold the answer so the caller can choose another boundary witness.
fn contains(ring: &[Segment], point: P, utol: f64) -> Option<bool> {
    let mut inside = false;
    for s in ring {
        let [lo, hi] = s.bounds();
        if s.has(point, utol) || (point[0] - lo).abs() <= utol || (point[0] - hi).abs() <= utol {
            return None;
        }
        if s.graph.is_some() && point[0] > lo && point[0] < hi && s.value(point[0]) > point[1] {
            inside = !inside;
        }
    }
    Some(inside)
}

/// Intersections have already been excluded, so any unambiguous point on the
/// candidate ring determines containment of that entire simple ring.
fn contains_ring(container: &[Segment], candidate: &[Segment], utol: f64) -> Option<bool> {
    candidate.iter().find_map(|s| {
        let u = s.start[0].midpoint(s.end[0]);
        let v = if s.graph.is_some() {
            s.value(u)
        } else {
            s.start[1].midpoint(s.end[1])
        };
        contains(container, [u, v], utol).or_else(|| contains(container, s.start, utol))
    })
}

fn classify(charts: &mut [Chart], radius: f64, sign: f64) -> Option<Vec<f64>> {
    if charts.is_empty()
        || charts.len() > 128
        || charts.iter().map(|c| c.ring.len()).sum::<usize>() > 512
    {
        return None;
    }
    let areas = charts
        .iter_mut()
        .map(|c| chart_area(&mut c.ring, radius, sign, c.cut_seam))
        .collect::<Option<Vec<_>>>()?;
    let mut outer = areas.iter().enumerate().filter(|(_, area)| **area > 0.0);
    let (outer_index, _) = outer.next()?;
    if outer.next().is_some() {
        return None;
    }
    let utol = TOL / radius;
    let [low, high] = bounds(&charts[outer_index].ring);
    for (i, chart) in charts.iter_mut().enumerate() {
        if i == outer_index {
            continue;
        }
        if chart.cut_seam {
            return None;
        }
        let [lo, hi] = bounds(&chart.ring);
        // A proper hole must fit strictly inside one copy of the outer chart.
        // Since the outer spans at most one period, that copy is unique.
        let shift = ((low.midpoint(high) - lo.midpoint(hi)) / TAU).round() * TAU;
        if lo + shift <= low + utol || hi + shift >= high - utol {
            return None;
        }
        for s in &mut chart.ring {
            s.start[0] += shift;
            s.end[0] += shift;
        }
    }
    for (i, a) in charts.iter().enumerate() {
        for (j, b) in charts.iter().enumerate().skip(i + 1) {
            for left in &a.ring {
                for right in &b.ring {
                    if !crossings(left, right, utol)?.is_empty() {
                        return None;
                    }
                }
            }
            if contains_ring(&a.ring, &b.ring, utol)? != (i == outer_index)
                || contains_ring(&b.ring, &a.ring, utol)? != (j == outer_index)
            {
                return None;
            }
        }
    }
    (areas.iter().sum::<f64>() > TOL * TOL).then_some(areas)
}

fn segment(pc: &PcurveGeometry, range: [f64; 2], radius: f64) -> Option<Segment> {
    let mut s = match *pc {
        PcurveGeometry::Line { origin, direction } => {
            if direction.u.abs() > 1e-12 && direction.v.abs() > 1e-12 {
                return None;
            }
            let point = |t| [origin.u + t * direction.u, origin.v + t * direction.v];
            Segment {
                start: point(range[0]),
                end: point(range[1]),
                graph: (direction.u.abs() > 1e-12).then_some([origin.v, 0., 0.]),
            }
        }
        PcurveGeometry::PolarHarmonic {
            radial_center,
            radial_cos,
            radial_sin,
            axial_origin,
            axial_cos,
            axial_sin,
        } => {
            if radial_center.u.hypot(radial_center.v) > TOL
                || (radial_cos.u.hypot(radial_cos.v) - radius).abs() > TOL
                || (radial_sin.u.hypot(radial_sin.v) - radius).abs() > TOL
                || (radial_cos.u * radial_sin.u + radial_cos.v * radial_sin.v).abs() > TOL * radius
            {
                return None;
            }
            let phase = radial_cos.v.atan2(radial_cos.u);
            let sense = (radial_cos.u * radial_sin.v - radial_cos.v * radial_sin.u).signum();
            let point = |t: f64| {
                [
                    phase + sense * t,
                    axial_origin + axial_cos * t.cos() + axial_sin * t.sin(),
                ]
            };
            Segment {
                start: point(range[0]),
                end: point(range[1]),
                graph: Some([
                    axial_origin,
                    axial_cos * phase.cos() - sense * axial_sin * phase.sin(),
                    axial_cos * phase.sin() + sense * axial_sin * phase.cos(),
                ]),
            }
        }
        _ => return None,
    };
    if s.start
        .iter()
        .chain(s.end.iter())
        .chain(s.graph.iter().flatten())
        .any(|v| !v.is_finite())
        || (s.end[0] - s.start[0]).abs() > TAU + TOL / radius
        || ((s.start[0] - s.end[0]).abs() * radius <= TOL && (s.start[1] - s.end[1]).abs() <= TOL)
    {
        return None;
    }
    // Keep the initial phase small before sequential chart unwrapping.
    let shift = (s.start[0] / TAU).floor() * TAU;
    s.start[0] -= shift;
    s.end[0] -= shift;
    Some(s)
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
    let tag = |id: &str| {
        annotations
            .provenance
            .get(id)
            .and_then(|p| p.tag.as_deref())
    };
    let mut result = BTreeMap::new();
    for face in &ir.model.faces {
        let evidence = (|| {
            let SurfaceGeometry::Cylinder { radius, .. } = surfaces.get(&face.surface)?.geometry
            else {
                return None;
            };
            if !sources.native_entity(face.id.as_str())
                || face.loops.is_empty()
                || face.loops.len() > 128
                || face.loops.iter().collect::<BTreeSet<_>>().len() != face.loops.len()
            {
                return None;
            }
            let mut charts = Vec::new();
            let mut total_coedges = 0;
            for loop_id in &face.loops {
                let lp = *loops.get(loop_id)?;
                total_coedges += lp.coedges.len();
                if lp.face != face.id
                    || !lp.vertex_uses.is_empty()
                    || lp.coedges.len() < 4
                    || lp.coedges.len() > 128
                    || total_coedges > 512
                    || lp.boundary_role != cadmpeg_ir::topology::LoopBoundaryRole::Unspecified
                {
                    return None;
                }
                let mut seen = BTreeSet::new();
                let mut seam = Vec::new();
                let mut ring = Vec::new();
                for (i, id) in lp.coedges.iter().enumerate() {
                    let c = *coedges.get(id)?;
                    if !seen.insert(id)
                        || c.owner_loop != lp.id
                        || c.use_curve.is_some()
                        || c.use_curve_parameter_range.is_some()
                        || c.next != lp.coedges[(i + 1) % lp.coedges.len()]
                        || c.previous != lp.coedges[(i + lp.coedges.len() - 1) % lp.coedges.len()]
                    {
                        return None;
                    }
                    let edge = *edges.get(&c.edge)?;
                    let mut range = edge.param_range.or_else(|| {
                        intervals::derive_for_edge(edge, sources).map(|r| r.parameter_range)
                    })?;
                    if c.sense == Sense::Reversed {
                        range.swap(0, 1);
                    }
                    let [use_] = c.pcurves.as_slice() else {
                        return None;
                    };
                    let pc = *pcurves.get(&use_.pcurve)?;
                    if tag(pc.id.as_str()) != Some("derived_cylindrical_pcurve")
                        || use_.parameter_range.is_some()
                        || pc.parameter_range.is_some()
                        || pc.wrapper_reversed.is_some()
                    {
                        return None;
                    }
                    let segment = segment(&pc.geometry, range, radius)?;
                    if tag(edge.id.as_str()) == Some("derived_periodic_seam") {
                        if segment.graph.is_some() {
                            return None;
                        }
                        seam.push((edge.id.clone(), c.sense));
                    }
                    ring.push(segment);
                }
                let cut_seam = match seam.as_slice() {
                    [] => false,
                    [(a, sa), (b, sb)] if a == b && sa != sb => true,
                    _ => return None,
                };
                charts.push(Chart { ring, cut_seam });
            }
            let sign = if face.sense == Sense::Forward {
                1.
            } else {
                -1.
            };
            classify(&mut charts, radius, sign)
        })();
        if let Some(areas) = evidence {
            for (id, area) in face.loops.iter().zip(areas) {
                result.insert(
                    id.0.clone(),
                    GeometryDerivedLoopRole {
                        role: if area > 0.0 { "outer" } else { "inner" }.into(),
                        method: GeometryLoopRoleMethod::CylindricalAnalyticChart,
                        signed_area_mm2: area,
                        tolerance_mm: TOL,
                    },
                );
            }
        }
    }
    result
}

#[cfg(test)]
mod tests;
