//! Unit and IR conversion for the shared partial spline readers.
use super::{Carrier, CarrierGeometry, LEN_TO_MM};
use cadmpeg_ir::geometry::{CurveGeometry, NurbsCurve, NurbsSurface, SurfaceGeometry};
use cadmpeg_ir::math::Point3;
use parasolid_core::partial::spline::{self, SplineCarrier, SplineCurve, SplineSurface};
use std::collections::HashMap;

fn carrier<T>(value: SplineCarrier<T>, convert: impl FnOnce(T) -> CarrierGeometry) -> Carrier {
    Carrier {
        attr: value.attr,
        offset: value.offset,
        end: value.end,
        read_spans: value.read_spans.into_iter().map(Into::into).collect(),
        geometry: convert(value.geometry),
        frame: None,
        parameter_range: None,
        orientation_reversed: false,
    }
}
fn points(values: Vec<[f64; 3]>) -> Vec<Point3> {
    values
        .into_iter()
        .map(|p| Point3::new(p[0] * LEN_TO_MM, p[1] * LEN_TO_MM, p[2] * LEN_TO_MM))
        .collect()
}
pub fn scan_curve_carriers(bytes: &[u8]) -> HashMap<u16, Carrier> {
    spline::scan_curve_carriers(bytes)
        .into_iter()
        .map(|(id, value)| {
            (
                id,
                carrier(value, |curve| {
                    CarrierGeometry::Curve(CurveGeometry::Nurbs(NurbsCurve {
                        degree: curve.degree,
                        knots: curve.knots,
                        control_points: points(curve.control_points),
                        weights: curve.weights,
                        periodic: curve.periodic,
                    }))
                }),
            )
        })
        .collect()
}
pub fn scan_surface_carriers(bytes: &[u8]) -> HashMap<u16, Carrier> {
    spline::scan_surface_carriers(bytes)
        .into_iter()
        .map(|(id, value)| {
            (
                id,
                carrier(value, |surface| {
                    CarrierGeometry::Surface(SurfaceGeometry::Nurbs(NurbsSurface {
                        u_degree: surface.u_degree,
                        v_degree: surface.v_degree,
                        u_knots: surface.u_knots,
                        v_knots: surface.v_knots,
                        u_count: surface.u_count,
                        v_count: surface.v_count,
                        control_points: points(surface.control_points),
                        weights: surface.weights,
                        u_periodic: surface.u_periodic,
                        v_periodic: surface.v_periodic,
                    }))
                }),
            )
        })
        .collect()
}
fn curve_values(curve: &NurbsCurve) -> SplineCurve {
    SplineCurve {
        degree: curve.degree,
        knots: curve.knots.clone(),
        control_points: curve
            .control_points
            .iter()
            .map(|p| [p.x, p.y, p.z])
            .collect(),
        weights: curve.weights.clone(),
        periodic: curve.periodic,
    }
}
fn surface_values(surface: &NurbsSurface) -> SplineSurface {
    SplineSurface {
        u_degree: surface.u_degree,
        v_degree: surface.v_degree,
        u_knots: surface.u_knots.clone(),
        v_knots: surface.v_knots.clone(),
        u_count: surface.u_count,
        v_count: surface.v_count,
        control_points: surface
            .control_points
            .iter()
            .map(|p| [p.x, p.y, p.z])
            .collect(),
        weights: surface.weights.clone(),
        u_periodic: surface.u_periodic,
        v_periodic: surface.v_periodic,
    }
}
pub(crate) fn patch_nurbs_curve(
    bytes: &mut [u8],
    offset: usize,
    old: &NurbsCurve,
    new: &NurbsCurve,
    scale: f64,
) -> Option<()> {
    spline::patch_nurbs_curve(bytes, offset, &curve_values(old), &curve_values(new), scale)
}
pub(crate) fn patch_nurbs_surface(
    bytes: &mut [u8],
    offset: usize,
    old: &NurbsSurface,
    new: &NurbsSurface,
    scale: f64,
) -> Option<()> {
    spline::patch_nurbs_surface(
        bytes,
        offset,
        &surface_values(old),
        &surface_values(new),
        scale,
    )
}
