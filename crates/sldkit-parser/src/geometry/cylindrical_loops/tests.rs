use super::*;
use cadmpeg_ir::{
    AnnotationBuilder, SourceFidelity,
    geometry::{CurveGeometry, Pcurve},
    math::{Point2, Point3, Vector3},
    topology::{BodyKind, LoopBoundaryRole, PcurveUse},
};

/// Synthetic, geometrically consistent cylinder patch with a rectangular hole.
/// Provenance markers exercise the adapter contract, not native file parsing.
#[allow(clippy::too_many_lines)]
fn public_fixture() -> (CadIr, SourceFidelity) {
    let mut ir = cadmpeg_ir::examples::unit_cube();
    ir.model.bodies[0].kind = BodyKind::Sheet;
    ir.model.faces.truncate(1);
    ir.model.surfaces.truncate(1);
    ir.model.loops.truncate(2);
    ir.model.coedges.truncate(8);
    ir.model.edges.truncate(8);
    ir.model.curves.truncate(8);
    ir.model.shells[0].faces = vec![ir.model.faces[0].id.clone()];
    ir.model.faces[0].sense = Sense::Forward;
    ir.model.faces[0].loops = ir
        .model
        .loops
        .iter()
        .rev()
        .map(|lp| lp.id.clone())
        .collect();
    ir.model.surfaces[0].geometry = SurfaceGeometry::Cylinder {
        origin: Point3::new(0., 0., 0.),
        axis: Vector3::new(0., 0., 1.),
        ref_direction: Vector3::new(1., 0., 0.),
        radius: 2.,
    };
    let mut annotations = AnnotationBuilder::new();
    let stream = annotations.stream("synthetic:native-profile");
    annotations
        .note(&ir.model.bodies[0].id, stream, 0)
        .tag("00_0c_body");
    annotations
        .note(&ir.model.faces[0].id, stream, 0)
        .tag("00_0e");
    for (index, chart) in [
        box_chart([0., 0.], [5., 10.], false),
        box_chart([1., 1.], [2., 3.], true),
    ]
    .iter()
    .enumerate()
    {
        let lp = &mut ir.model.loops[index];
        lp.face = ir.model.faces[0].id.clone();
        lp.boundary_role = LoopBoundaryRole::Unspecified;
        for (j, s) in chart.ring.iter().enumerate() {
            let i = 4 * index + j;
            let c = &mut ir.model.coedges[i];
            c.sense = Sense::Forward;
            c.edge = ir.model.edges[i].id.clone();
            c.radial_next = c.id.clone();
            ir.model.points[i].position =
                Point3::new(2. * s.start[0].cos(), 2. * s.start[0].sin(), s.start[1]);
            ir.model.edges[i].start = ir.model.vertices[i].id.clone();
            ir.model.edges[i].end = ir.model.vertices[4 * index + (j + 1) % 4].id.clone();
            let horizontal = s.graph.is_some();
            ir.model.edges[i].param_range = Some(if horizontal {
                [s.start[0], s.end[0]]
            } else {
                [s.start[1], s.end[1]]
            });
            ir.model.curves[i].geometry = if horizontal {
                CurveGeometry::Circle {
                    center: Point3::new(0., 0., s.start[1]),
                    axis: Vector3::new(0., 0., 1.),
                    ref_direction: Vector3::new(1., 0., 0.),
                    radius: 2.,
                }
            } else {
                CurveGeometry::Line {
                    origin: Point3::new(2. * s.start[0].cos(), 2. * s.start[0].sin(), 0.),
                    direction: Vector3::new(0., 0., 1.),
                }
            };
            let id = cadmpeg_ir::ids::PcurveId(format!("synthetic:pc:{i}"));
            c.pcurves = vec![PcurveUse {
                pcurve: id.clone(),
                isoparametric: None,
                parameter_range: None,
            }];
            annotations
                .note(&id, stream, 0)
                .tag("derived_cylindrical_pcurve");
            ir.model.pcurves.push(Pcurve {
                id,
                geometry: PcurveGeometry::Line {
                    origin: if horizontal {
                        Point2::new(0., s.start[1])
                    } else {
                        Point2::new(s.start[0], 0.)
                    },
                    direction: if horizontal {
                        Point2::new(1., 0.)
                    } else {
                        Point2::new(0., 1.)
                    },
                },
                wrapper_reversed: None,
                native_tail_flags: None,
                parameter_range: None,
                fit_tolerance: None,
            });
        }
    }
    let mut fidelity = SourceFidelity::default();
    fidelity.annotations = annotations.build();
    (ir, fidelity)
}

#[test]
fn public_mapping_keeps_source_roles_and_intervals_separate()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut ir, fidelity) = public_fixture();
    let model = super::super::map_model(&ir, &fidelity)?;
    let roles = model
        .loops
        .iter()
        .map(|lp| lp.derived_boundary_role.as_ref().ok_or("missing role"))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(roles[0].role, "outer");
    assert_eq!(roles[1].role, "inner");
    // OCP BRepGProp integration of the independently constructed trimmed
    // cylindrical face: 100 mm2 outer minus 4 mm2 hole.
    assert!((roles.iter().map(|r| r.signed_area_mm2).sum::<f64>() - 96.).abs() < 1e-12);
    let json = serde_json::to_value(&model)?;
    assert_eq!(
        json["loops"][1]["derived_boundary_role"]["method"],
        "cylindrical_analytic_chart"
    );
    assert!(
        model
            .loops
            .iter()
            .all(|lp| lp.boundary_role == "unspecified")
    );
    assert!(
        model
            .edges
            .iter()
            .all(|edge| edge.derived_parameter_interval.is_none())
    );
    // Reverse the face and every coedge traversal together.
    ir.model.faces[0].sense = Sense::Reversed;
    for lp in &mut ir.model.loops {
        lp.coedges.reverse();
    }
    for c in &mut ir.model.coedges {
        c.sense = Sense::Reversed;
        std::mem::swap(&mut c.next, &mut c.previous);
    }
    let reversed = super::super::map_model(&ir, &fidelity)?;
    assert_eq!(
        model.loops[1].derived_boundary_role,
        reversed.loops[1].derived_boundary_role
    );
    Ok(())
}

#[test]
fn damaged_or_unsupported_member_withholds_the_entire_face()
-> Result<(), Box<dyn std::error::Error>> {
    for damage in 0..7 {
        let (mut ir, mut fidelity) = public_fixture();
        match damage {
            0 => ir.model.coedges[4].next = ir.model.coedges[4].id.clone(),
            1 => ir.model.loops[1].boundary_role = LoopBoundaryRole::Inner,
            2 => ir.model.coedges[4].use_curve = ir.model.edges[4].curve.clone(),
            3 => ir.model.coedges[4].pcurves[0].parameter_range = Some([0., 1.]),
            4 => ir.model.pcurves[4].wrapper_reversed = Some(true),
            5 => ir.model.faces[0].loops.push(ir.model.loops[0].id.clone()),
            _ => fidelity.annotations.provenance.clear(),
        }
        let model = super::super::map_model(&ir, &fidelity)?;
        assert!(
            model
                .loops
                .iter()
                .all(|lp| lp.derived_boundary_role.is_none()),
            "damage {damage}"
        );
    }
    Ok(())
}

fn rectangle(width: f64) -> Vec<Segment> {
    vec![
        Segment {
            start: [0., 0.],
            end: [width, 0.],
            graph: Some([0., 0., 0.]),
        },
        Segment {
            start: [width, 0.],
            end: [width, 3.],
            graph: None,
        },
        Segment {
            start: [width, 3.],
            end: [0., 3.],
            graph: Some([3., 0., 0.]),
        },
        Segment {
            start: [0., 3.],
            end: [0., 0.],
            graph: None,
        },
    ]
}
#[test]
fn chart_crosses_branch_and_requires_explicit_full_period_seams() -> Result<(), &'static str> {
    let mut ring = rectangle(2.);
    for s in &mut ring[1..] {
        s.start[0] -= TAU;
        s.end[0] -= TAU;
    }
    assert!((chart_area(&mut ring, 5., 1., false).ok_or("closed chart")? - 30.).abs() < 1e-10);
    assert!(chart_area(&mut rectangle(TAU), 5., 1., false).is_none());
    assert!(chart_area(&mut rectangle(TAU), 5., 1., true).is_some());
    assert!(chart_area(&mut rectangle(TAU + 0.1), 5., 1., true).is_none());
    assert!(chart_area(&mut rectangle(2.), 5., -1., false).is_some_and(|a| a < 0.));
    Ok(())
}
#[test]
fn harmonic_crossings_and_overlapping_boundaries_are_rejected() {
    let mut ring = rectangle(4.);
    ring[0].graph = Some([1., 2., 0.]);
    ring[0].start[1] = 3.;
    ring[0].end[1] = 1. + 2. * 4_f64.cos();
    ring[1].start = ring[0].end;
    ring[3].end = ring[0].start;
    assert!(chart_area(&mut ring, 1., 1., false).is_none());
    let a = rectangle(2.).remove(0);
    assert!(crossings(&a, &a, 1e-7).is_none());
    let mut open = rectangle(2.);
    open[1].start[1] = 0.1;
    assert!(chart_area(&mut open, 1., 1., false).is_none());
}

fn reverse(chart: &mut Chart) {
    chart.ring.reverse();
    for s in &mut chart.ring {
        std::mem::swap(&mut s.start, &mut s.end);
    }
}

fn box_chart(low: P, high: P, inner: bool) -> Chart {
    let mut chart = Chart {
        ring: rectangle(high[0] - low[0]),
        cut_seam: false,
    };
    for s in &mut chart.ring {
        for p in [&mut s.start, &mut s.end] {
            p[0] += low[0];
            p[1] = low[1] + p[1] * (high[1] - low[1]) / 3.;
        }
        if let Some(graph) = &mut s.graph {
            graph[0] = s.start[1];
        }
    }
    if inner {
        reverse(&mut chart);
    }
    chart
}

#[test]
fn holes_ignore_list_order_phase_branch_and_follow_face_sense() -> Result<(), &'static str> {
    let mut charts = [
        box_chart([6., 1.], [7., 3.], true),
        box_chart([5.5, 0.], [10.5, 10.], false),
        box_chart([8., 6.], [9., 8.], true),
    ];
    // atan2 branches can differ even within a ring.
    for c in &mut charts {
        for (i, s) in (0..4).zip(&mut c.ring) {
            s.start[0] -= f64::from(i) * TAU;
            s.end[0] -= f64::from(i) * TAU;
        }
    }
    let areas = classify(&mut charts, 2., 1.).ok_or("valid holes")?;
    assert_eq!(areas, [-4., 100., -4.]);
    for c in &mut charts {
        reverse(c);
    }
    assert_eq!(
        classify(&mut charts, 2., -1.).ok_or("reversed face")?,
        areas
    );
    assert!(classify(&mut charts, 2., 1.).is_none());
    Ok(())
}

#[test]
fn full_period_chart_accepts_holes_only_clear_of_its_explicit_cut() -> Result<(), &'static str> {
    let outer = || {
        let mut c = box_chart([0., 0.], [TAU, 10.], false);
        c.cut_seam = true;
        c
    };
    let areas = classify(
        &mut [outer(), box_chart([0.5 + TAU, 1.], [1.5 + TAU, 3.], true)],
        2.,
        1.,
    )
    .ok_or("full period with hole")?;
    assert!((areas[0] - 20. * TAU).abs() < 1e-12);
    assert!((areas[1] + 4.).abs() < 1e-12);
    for hole in [
        box_chart([TAU - 0.5, 1.], [TAU + 0.5, 3.], true),
        box_chart([0., 1.], [1., 3.], true),
    ] {
        assert!(classify(&mut [outer(), hole], 2., 1.).is_none());
    }
    Ok(())
}

#[test]
fn touching_crossing_outside_nested_and_periodic_duplicate_holes_are_rejected() {
    for holes in [
        vec![box_chart([1., -2.], [2., -1.], true)],
        vec![box_chart([1., 0.], [2., 3.], true)],
        vec![box_chart([1., -1.], [2., 3.], true)],
        vec![box_chart([1., 1.], [2., 3.], false)],
        vec![
            box_chart([1., 1.], [4., 8.], true),
            box_chart([2., 2.], [3., 3.], true),
        ],
        vec![
            box_chart([1., 1.], [3., 3.], true),
            box_chart([2., 2.], [4., 4.], true),
        ],
        vec![
            box_chart([1., 1.], [2., 3.], true),
            box_chart([2., 1.], [3., 3.], true),
        ],
        vec![
            box_chart([1., 1.], [2., 3.], true),
            box_chart([1. + TAU, 1.], [2. + TAU, 3.], true),
        ],
    ] {
        let mut charts = vec![box_chart([0., 0.], [5., 10.], false)];
        charts.extend(holes);
        assert!(classify(&mut charts, 2., 1.).is_none());
    }
}

#[test]
fn harmonic_hole_area_uses_the_analytic_section() -> Result<(), &'static str> {
    let mut hole = box_chart([1., 1.], [2., 3.], false);
    hole.ring[0].graph = Some([1., 0.2, 0.]);
    hole.ring[0].start[1] = 1. + 0.2 * 1_f64.cos();
    hole.ring[0].end[1] = 1. + 0.2 * 2_f64.cos();
    hole.ring[1].start = hole.ring[0].end;
    hole.ring[3].end = hole.ring[0].start;
    reverse(&mut hole);
    let areas = classify(&mut [box_chart([0., 0.], [4., 4.], false), hole], 3., 1.)
        .ok_or("elliptical section hole")?;
    assert!((areas[1] + 3. * (2. - 0.2 * (2_f64.sin() - 1_f64.sin()))).abs() < 1e-12);
    Ok(())
}

#[test]
fn face_work_limits_and_invalid_radius_withhold_roles() {
    assert!(classify(&mut [], 1., 1.).is_none());
    for count in [129, 513] {
        let mut charts = (0..count)
            .map(|_| box_chart([0., 0.], [1., 1.], false))
            .collect::<Vec<_>>();
        assert!(classify(&mut charts, 1., 1.).is_none());
    }
    for radius in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(classify(&mut [box_chart([0., 0.], [1., 1.], false)], radius, 1.).is_none());
    }
}
