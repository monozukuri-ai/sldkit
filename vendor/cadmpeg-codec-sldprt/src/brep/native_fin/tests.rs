// SPDX-License-Identifier: Apache-2.0
// Modified by sldkit; see PATCHES.md.
//! Synthetic source graphs, independent of private fixture geometry.
use super::*;
use crate::brep::{topology::Record, Carrier};
use cadmpeg_ir::math::{Point3, Vector3};

fn record(attr: u16, refs: Vec<u16>, marker: Option<u8>) -> Record {
    Record {
        attr,
        refs,
        marker,
        xyz_m: None,
        xyz_offset: None,
        owner: None,
        offset: usize::from(attr) * 32,
        read_ranges: vec![(usize::from(attr) * 32, usize::from(attr) * 32 + 2)],
    }
}

fn ring_pair(tables: &mut Tables, shift: u16, vertex: u16) {
    for (id, other, loop_, face, marker) in [(20, 21, 30, 40, b'+'), (21, 20, 31, 41, b'-')] {
        let (id, other, loop_, face) = (id + shift, other + shift, loop_ + shift, face + shift);
        tables.coedges.insert(
            id,
            record(
                id,
                vec![1, loop_, id, id, vertex, other, 10 + shift, 1, 1],
                Some(marker),
            ),
        );
        tables
            .loops
            .insert(loop_, record(loop_, vec![1, id, face, 1], None));
        tables
            .bridges
            .insert(face, record(face, vec![1, 1, loop_, 1, 1], Some(b'+')));
    }
    tables.edge_uses.insert(
        10 + shift,
        record(10 + shift, vec![20 + shift, 1, 1, 50, 1, 1], None),
    );
}

fn fixture() -> (Tables, CarrierIndex) {
    let mut tables = Tables::default();
    ring_pair(&mut tables, 0, 1);
    let mut carriers = CarrierIndex::default();
    carriers.insert(Carrier {
        attr: 50,
        offset: 500,
        end: 600,
        read_spans: Vec::new(),
        geometry: CarrierGeometry::Curve(CurveGeometry::Circle {
            center: Point3::new(0.0, 0.0, 0.0),
            axis: Vector3::new(0.0, 0.0, 1.0),
            ref_direction: Vector3::new(1.0, 0.0, 0.0),
            radius: 2.0,
        }),
        frame: None,
        parameter_range: None,
        orientation_reversed: false,
    });
    (tables, carriers)
}

#[test]
fn vertexless_circle_keeps_null_vertices_source_offsets_and_spans() {
    let (mut tables, carriers) = fixture();
    let source = tables.coedges.clone();
    assert_eq!(normalize_or_withhold(&mut tables, &carriers), 0);
    assert!(tables.vertex_uses.is_empty());
    assert!(tables.points.is_empty());
    for (id, before) in source {
        let after = &tables.coedges[&id];
        assert_eq!(after.refs, before.refs);
        assert_eq!(after.marker, before.marker);
        assert_eq!(after.offset, before.offset);
        assert_eq!(after.read_ranges, before.read_ranges);
    }
}

#[test]
fn a_circle_sheet_can_have_one_dummy_fin() {
    let (mut tables, carriers) = fixture();
    tables.coedges.get_mut(&21).unwrap().refs[1..4].fill(1);
    assert_eq!(normalize_or_withhold(&mut tables, &carriers), 0);
    assert_eq!(tables.coedges.len(), 2);
}

#[test]
fn mixed_ring_and_ordinary_fins_still_use_the_core_validator() {
    let (mut tables, carriers) = fixture();
    ring_pair(&mut tables, 100, 60);
    tables.vertex_uses.insert(60, record(60, vec![], None));
    assert_eq!(normalize_or_withhold(&mut tables, &carriers), 0);
    assert_eq!(tables.coedges.len(), 4);
    assert_eq!(tables.coedges[&120].refs[4], 60);

    // A dangling ordinary endpoint withholds the entire arena, including rings.
    let (mut tables, carriers) = fixture();
    ring_pair(&mut tables, 100, 60);
    assert_eq!(normalize_or_withhold(&mut tables, &carriers), 4);
    assert!(tables.coedges.is_empty());
}

#[test]
fn invalid_circle_links_never_bypass_native_fin_validation() {
    for case in 0..11 {
        let (mut tables, carriers) = fixture();
        match case {
            0 => tables.coedges.get_mut(&20).unwrap().refs[5] = 999,
            1 => tables.coedges.get_mut(&21).unwrap().refs[6] = 999,
            2 => tables.coedges.get_mut(&21).unwrap().marker = Some(b'+'),
            3 => tables.coedges.get_mut(&21).unwrap().refs[4] = 60,
            4 => tables.coedges.get_mut(&20).unwrap().refs[2] = 21,
            5 => {
                tables.bridges.remove(&40);
            }
            6 => tables.coedges.get_mut(&20).unwrap().refs[7] = 50,
            7 => tables.coedges.get_mut(&20).unwrap().refs[8] = 21,
            8 => {
                for fin in tables.coedges.values_mut() {
                    fin.refs[1..4].fill(1);
                }
            }
            9 => tables.coedges.get_mut(&20).unwrap().refs[4] = 0,
            10 => tables.loops.get_mut(&30).unwrap().refs[1] = 21,
            _ => unreachable!(),
        }
        assert_eq!(
            normalize_or_withhold(&mut tables, &carriers),
            2,
            "case {case}"
        );
        assert!(tables.coedges.is_empty(), "case {case}");
    }
}

#[test]
fn unsupported_or_bounded_curves_do_not_prove_a_circle_ring() {
    for case in 0..4 {
        let (mut tables, mut carriers) = fixture();
        match case {
            0 => {
                carriers.curves.get_mut(&50).unwrap().geometry =
                    CarrierGeometry::Curve(CurveGeometry::Line {
                        origin: Point3::new(0.0, 0.0, 0.0),
                        direction: Vector3::new(1.0, 0.0, 0.0),
                    })
            }
            1 => carriers.curves.get_mut(&50).unwrap().parameter_range = Some([0.0, 1.0]),
            2 => {
                carriers.curves.clear();
            }
            3 => {
                carriers.derived_curves.insert(50);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            normalize_or_withhold(&mut tables, &carriers),
            2,
            "case {case}"
        );
    }
}
