// SPDX-License-Identifier: Apache-2.0
// Modified by sldkit; see the crate-root PATCHES.md.
//! Synthetic record graphs; no private CAD geometry is embedded here.
#![allow(clippy::unwrap_used)]

use super::*;
const SCHEMA: &str = "SCH_3701229_37102_13006";
const BODY_DECLARATION: &[u8] = b"$CCCI\x07lattice\x00\xde\x00\x01CCCI\x04mesh\x03\xee\x00\x01I\x08polyline\x03\xf0\x00\x01CCCCCCCDI\x05owner\x04\x10\x00\x01CCCI\x10boundary_lattice\x00\xde\x00\x01CCCI\x0dboundary_mesh\x03\xee\x00\x01I\x11boundary_polyline\x03\xf0\x00\x01CCCA\x10index_map_offset\x00\x00\x00\x01\x01dA\x09index_map\x00R\x00\x01A\x11node_id_index_map\x00R\x00\x01A\x14schema_embedding_map\x00R\x00\x01A\x05child\x00\x0c\x00\x01A\x0elowest_node_id\x00\x00\x00\x01\x01dA\x10mesh_offset_data\x00\xce\x00\x01Z";
const REGION_DECLARATION: &[u8] =
    b"\x09CCCCCCI\x05frame\x00\xe6\x00\x01CA\x05owner\x00\x0c\x00\x01Z";
use crate::brep::topology::Record;

fn u16_at(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_be_bytes());
}

fn append(bytes: &mut Vec<u8>, tag: u8, declaration: &[u8], raw: &[u8]) -> usize {
    bytes.extend([0, tag]);
    bytes.extend(declaration);
    let start = bytes.len();
    bytes.extend(raw);
    start
}

fn fixture() -> (Vec<u8>, Tables, Vec<usize>) {
    let mut data = Vec::new();
    let mut tables = Tables::default();
    let mut body_starts = Vec::new();
    // A solid and a sheet. No coordinates or connected-component heuristics
    // are available; the native graph is the sole membership evidence.
    for (index, (id, kind)) in [(3, 1), (33, 3)].into_iter().enumerate() {
        let region_id = 101 + index as u16 * 10;
        let shell_id = 201 + index as u16 * 10;
        let face_id = 301 + index as u16 * 10;
        let mut raw = [0_u8; 89];
        u16_at(&mut raw, 0, id);
        raw[5] = 5;
        raw[24..32].copy_from_slice(&1_f64.to_be_bytes());
        raw[32..40].copy_from_slice(&1e-8_f64.to_be_bytes());
        u16_at(&mut raw, 42, if index == 0 { 33 } else { 1 });
        u16_at(&mut raw, 44, if index == 0 { 1 } else { 3 });
        raw[46] = 1;
        u16_at(&mut raw, 47, 2);
        raw[49] = kind;
        raw[50] = 1;
        u16_at(&mut raw, 51, shell_id);
        u16_at(&mut raw, 65, region_id);
        u16_at(&mut raw, 87, 1);
        body_starts.push(append(
            &mut data,
            12,
            if index == 0 { BODY_DECLARATION } else { &[] },
            &raw,
        ));

        let mut raw = [0_u8; 21];
        for (at, value) in [
            (0, region_id),
            (8, id),
            (10, 1),
            (12, 1),
            (14, shell_id),
            (16, 1),
            (19, 1),
        ] {
            u16_at(&mut raw, at, value);
        }
        raw[5] = 5;
        raw[18] = if kind == 1 { b'S' } else { b'V' };
        append(
            &mut data,
            19,
            if index == 0 { REGION_DECLARATION } else { &[] },
            &raw,
        );

        let mut raw = [0_u8; 22];
        for (at, value) in [
            (0, shell_id),
            (8, id),
            (10, 1),
            (12, face_id),
            (14, 1),
            (16, 1),
            (18, region_id),
            (20, face_id),
        ] {
            u16_at(&mut raw, at, value);
        }
        raw[5] = 5;
        append(&mut data, 13, &[], &raw);
        tables.bridges.insert(
            face_id,
            Record {
                read_ranges: Vec::new(),
                attr: face_id,
                refs: vec![1, 1, 1, shell_id, 1],
                marker: None,
                xyz_m: None,
                xyz_offset: None,
                owner: None,
                offset: 0,
            },
        );
    }
    (data, tables, body_starts)
}

fn native_sheet_body() -> Vec<u8> {
    use crate::{brep, test_support};
    let (mut data, _, starts) = fixture();
    data.truncate(starts[1] - 2);
    u16_at(&mut data, starts[0] + 42, 1);
    data[starts[0] + 49] = 3;
    let region = data
        .windows(REGION_DECLARATION.len())
        .position(|w| w == REGION_DECLARATION)
        .unwrap()
        + REGION_DECLARATION.len();
    data[region + 18] = b'V';
    let shell = region + 21 + 2;
    u16_at(&mut data, shell + 12, 10);
    u16_at(&mut data, shell + 20, 10);
    let mut triangle = test_support::triangle_body();
    let tables =
        brep::topology::scan_with_curve_attrs(&triangle, &std::collections::HashSet::default());
    let face = tables.bridges[&10].offset + 2;
    for (offset, value) in [(16, 1), (18, 1), (22, 201)] {
        u16_at(&mut triangle, face + offset, value);
    }
    // The generic writer fixture uses a start-vertex gauge and omits dummy
    // fins. Encode native FIN forward/backward/end vertices explicitly here.
    for i in 0..3_u16 {
        let fin = &tables.coedges[&(30 + i)];
        let p = fin.offset + 2;
        for (field, value) in [
            (2, 30 + (i + 1) % 3),
            (3, 30 + (i + 2) % 3),
            (4, 50 + (i + 1) % 3),
            (5, 70 + i),
        ] {
            u16_at(&mut triangle, p + 2 + field * 2, value);
        }
        triangle.extend(test_support::coedge(
            70 + i,
            1,
            1,
            50 + i,
            30 + i,
            40 + i,
            true,
        ));
    }
    data.extend(triangle);
    data
}

#[test]
fn decoded_graph_keeps_native_sheet_ids_and_explicit_source_annotations() {
    use crate::{brep, parasolid, test_support};
    let data = native_sheet_body();
    let stream = test_support::parasolid_with_body("partition body", SCHEMA, &data);
    let header = parasolid::stream_header(&stream).unwrap();
    let decoded = brep::decode_bodies(&[(&stream, &header)], "test-partition");
    assert!(!decoded.stats.synthetic_body_grouping);
    assert_eq!(decoded.stats.unresolved_native_fins, 0);
    assert_eq!(decoded.bodies.len(), 1);
    assert_eq!(decoded.bodies[0].kind, BodyKind::Sheet);
    assert_eq!(decoded.faces.len(), 1);
    assert_eq!(decoded.faces[0].shell.0, "sldprt:brep:shell#201");
    for (id, tag) in [
        ("body#3", "00_0c_body"),
        ("region#101", "00_13_region"),
        ("shell#201", "00_0d_shell"),
    ] {
        let id = format!("sldprt:brep:{id}");
        assert_eq!(
            decoded.annotations.provenance[&id].tag.as_deref(),
            Some(tag)
        );
        assert_eq!(
            decoded.annotations.exactness[&id].entity,
            cadmpeg_ir::Exactness::ByteExact
        );
    }
}

#[test]
fn rejected_native_fin_graph_keeps_display_cache_and_failure_reason() {
    use crate::{brep, test_support::*, SldprtCodec};
    use cadmpeg_ir::codec::{Codec, DecodeOptions};

    let mut body = native_sheet_body();
    let tables =
        brep::topology::scan_with_curve_attrs(&body, &std::collections::HashSet::default());
    // The forward endpoint cannot resolve to a vertex. Keep every other link.
    let forward_vertex = tables.coedges[&30].offset + 2 + 2 + 4 * 2;
    u16_at(&mut body, forward_vertex, 1);
    let stream = parasolid_with_body("partition body", SCHEMA, &body);
    let mut source = outer_header();
    source.extend(make_block(0x20, "Contents/Config-0-Partition", &stream));
    source.extend(make_block(
        0x41,
        "Contents/DisplayLists",
        &display_list_payload(),
    ));
    let decoded = SldprtCodec
        .decode(&mut std::io::Cursor::new(source), &DecodeOptions::default())
        .unwrap();
    assert!(!decoded.report().geometry_transferred);
    assert!(decoded.ir().model.bodies.is_empty());
    assert!(decoded.ir().model.faces.is_empty());
    assert_eq!(decoded.ir().model.tessellations.len(), 1);
    assert!(decoded.report().losses.iter().any(|loss| loss
        .message
        .starts_with("6 native FIN record(s) were withheld")));
}
