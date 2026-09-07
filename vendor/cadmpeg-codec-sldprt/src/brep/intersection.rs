//! Convert shared intersection caches to the existing derived IR geometry.
use super::{Carrier, CarrierGeometry, LEN_TO_MM};
use cadmpeg_ir::geometry::{CurveGeometry, NurbsCurve};
use cadmpeg_ir::math::{Point2, Point3};
use std::collections::HashMap;
pub(super) struct IntersectionCarrier {
    pub carrier: Carrier,
    pub support_data: IntersectionSupportData,
}
#[derive(Clone)]
pub(super) struct IntersectionSupportData {
    pub supports: [u16; 2],
    pub fit_tolerance_mm: f64,
    pub support_uv: Option<[Vec<Point2>; 2]>,
}
pub(super) fn scan_intersection_carriers(bytes: &[u8]) -> HashMap<u16, IntersectionCarrier> {
    parasolid_core::partial::intersection::scan_intersection_carriers_with_tolerance_filter(
        bytes,
        |tolerance| (tolerance * LEN_TO_MM).is_finite(),
    )
    .into_iter()
    .filter_map(|(id, value)| {
        let fit_tolerance_mm = value.support_data.fit_tolerance * LEN_TO_MM;
        if !fit_tolerance_mm.is_finite() {
            return None;
        }
        let record = value.carrier;
        let curve = record.geometry;
        Some((
            id,
            IntersectionCarrier {
                carrier: Carrier {
                    attr: record.attr,
                    offset: record.offset,
                    end: record.end,
                    read_spans: record.read_spans.into_iter().map(Into::into).collect(),
                    geometry: CarrierGeometry::Curve(CurveGeometry::Nurbs(NurbsCurve {
                        degree: curve.degree,
                        knots: curve.knots,
                        weights: curve.weights,
                        periodic: curve.periodic,
                        control_points: curve
                            .control_points
                            .into_iter()
                            .map(|p| {
                                Point3::new(p[0] * LEN_TO_MM, p[1] * LEN_TO_MM, p[2] * LEN_TO_MM)
                            })
                            .collect(),
                    })),
                    frame: None,
                    parameter_range: None,
                    orientation_reversed: false,
                },
                support_data: IntersectionSupportData {
                    supports: value.support_data.supports,
                    fit_tolerance_mm,
                    support_uv: value.support_data.support_uv.map(|sides| {
                        sides.map(|points| {
                            points
                                .into_iter()
                                .map(|p| Point2::new(p[0], p[1]))
                                .collect()
                        })
                    }),
                },
            },
        ))
    })
    .collect()
}
