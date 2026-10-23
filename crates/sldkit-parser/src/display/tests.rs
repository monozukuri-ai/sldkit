use super::{appearance, outline_edges, tables};

fn channel(width: u32, kind: u32, count: u32, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for value in [width, kind, 2, count] {
        out.extend(value.to_le_bytes());
    }
    out.extend(data);
    out
}

fn close(left: &[f32], right: &[f32]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| (a - b).abs() < 1e-6)
}

fn floats(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// One strip of `positions`, with stored normals unless `normals` is false.
fn table(positions: &[[f32; 3]], normals: bool) -> Vec<u8> {
    let count = u32::try_from(positions.len()).unwrap_or(0);
    let endpoints = count * 2 - 2;
    let flat = positions.iter().flatten().copied().collect::<Vec<_>>();
    let normal_values = if normals {
        positions
            .iter()
            .flat_map(|_| [0.0, 0.0, 1.0])
            .collect::<Vec<f32>>()
    } else {
        Vec::new()
    };
    let normal_count = u32::try_from(normal_values.len() / 3).unwrap_or(0);
    [
        channel(4, 8, 1, &count.to_le_bytes()),
        channel(12, 100, count, &floats(&flat)),
        channel(12, 100, normal_count, &floats(&normal_values)),
        channel(
            4,
            8,
            endpoints,
            &vec![0; usize::try_from(endpoints * 4).unwrap_or(0)],
        ),
        channel(4, 8, 1, &endpoints.to_le_bytes()),
        channel(
            1,
            8,
            endpoints,
            &vec![0; usize::try_from(endpoints).unwrap_or(0)],
        ),
    ]
    .concat()
}

fn class_declaration(name: &[u8]) -> Vec<u8> {
    let mut out = vec![0xff, 0xff, 0x01, 0x00];
    out.extend(u16::try_from(name.len()).unwrap_or(0).to_le_bytes());
    out.extend(name);
    out
}

fn face_header(triangles: u32, strips: u32) -> Vec<u8> {
    [triangles.to_le_bytes(), strips.to_le_bytes()].concat()
}

fn utf16_frame(text: &str) -> Vec<u8> {
    let units = text.encode_utf16().collect::<Vec<_>>();
    let mut out = vec![0xff, 0xfe, 0xff, u8::try_from(units.len()).unwrap_or(0)];
    out.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    out
}

const TRIANGLE: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [0.001, 0.0, 0.0], [0.0, 0.001, 0.0]];
const QUAD: [[f32; 3]; 4] = [
    [0.0, 0.0, 0.0],
    [0.001, 0.0, 0.0],
    [0.0, 0.001, 0.0],
    [0.001, 0.001, 0.0],
];

#[test]
fn declared_face_table_yields_strip_triangles() {
    let payload = [
        class_declaration(b"uoTempFaceTessData_c"),
        face_header(2, 1),
        table(&QUAD, true),
    ]
    .concat();
    let classes = tables::class_intervals(&payload);
    let faces = tables::faces(&payload, &classes);
    assert_eq!(faces.len(), 1);
    let triangles = faces[0].table.triangles().unwrap_or_default();
    assert_eq!(triangles, vec![[0, 1, 2], [1, 3, 2]]);
    assert_eq!(
        outline_edges(&faces[0].table.positions, &triangles).len(),
        4
    );
}

#[test]
fn inconsistent_auxiliary_channels_are_rejected() {
    let mut bytes = table(&TRIANGLE, true);
    // Corrupt the stored endpoint-list value (fifth channel payload).
    let at = bytes.len() - (16 + 4) - 4;
    bytes[at] = 9;
    let payload = [
        class_declaration(b"uoTempFaceTessData_c"),
        face_header(1, 1),
        bytes,
    ]
    .concat();
    let classes = tables::class_intervals(&payload);
    assert!(tables::faces(&payload, &classes).is_empty());
}

#[test]
fn reused_class_tag_admits_later_tables() {
    let first = table(&TRIANGLE, true);
    let second = table(&QUAD, true);
    let payload = [
        class_declaration(b"uoTempFaceTessData_c"),
        face_header(1, 1),
        first,
        vec![0x01, 0x80],
        face_header(2, 1),
        second,
    ]
    .concat();
    let classes = tables::class_intervals(&payload);
    let faces = tables::faces(&payload, &classes);
    assert_eq!(faces.len(), 2);
    assert_eq!(faces[1].table.positions.len(), 4);
}

#[test]
fn welded_duplicates_hide_the_shared_edge() {
    let positions = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let triangles = [[0, 1, 2], [3, 4, 5]];
    let edges = outline_edges(&positions, &triangles);
    assert_eq!(edges.len(), 4);
    assert!(!edges.contains(&[1, 2]));
}

#[test]
fn face_appearance_overrides_body_appearance() {
    let mut visual = vec![0x33, 0x80];
    visual.extend(0x0000_00ff_u32.to_le_bytes());
    visual.extend([0; 12]);
    visual.extend(utf16_frame("red"));
    let mut body = vec![0x33, 0x80];
    body.extend(0x00ff_0000_u32.to_le_bytes());
    body.extend([0; 12]);
    body.extend(utf16_frame("blue"));
    let payload = [
        class_declaration(b"uoTempFaceTessData_c"),
        face_header(1, 1),
        table(&TRIANGLE, true),
        visual,
        class_declaration(b"uoBodyPropInfo_c"),
        body,
    ]
    .concat();
    let classes = tables::class_intervals(&payload);
    let faces = tables::faces(&payload, &classes);
    let resolved = appearance::resolve(
        &payload,
        &classes,
        &faces,
        &std::collections::HashMap::new(),
    );
    let face = resolved[0].clone().unwrap_or_else(|| unreachable!());
    assert_eq!(face.name.as_deref(), Some("red"));
    assert!(close(&face.color, &[1.0, 0.0, 0.0, 1.0]));
    assert_eq!(face.source, sldkit_core::DisplayColorSource::Face);
}

#[test]
fn missing_normals_are_derived() {
    let payload = [
        class_declaration(b"uoTempFaceTessData_c"),
        face_header(1, 1),
        table(&TRIANGLE, false),
    ]
    .concat();
    let classes = tables::class_intervals(&payload);
    let faces = tables::faces(&payload, &classes);
    assert_eq!(faces.len(), 1);
    assert!(faces[0].table.normals.is_empty());
    let triangles = faces[0].table.triangles().unwrap_or_default();
    let normals = super::derived_normals(&faces[0].table.positions, &triangles);
    assert!(normals.iter().all(|normal| close(normal, &[0.0, 0.0, 1.0])));
}
