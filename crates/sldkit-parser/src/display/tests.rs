use super::{appearance, face_tessellations, outline_edges, tables};

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

const PLATE_ID: [u8; 8] = [0x8f, 0xea, 0x5b, 0x14, 0x44, 0xe2, 0x97, 0x01];
const PLATE_REPEAT_ID: [u8; 8] = [0x05, 0x19, 0xd1, 0x36, 0x8b, 0x6f, 0x04, 0x17];
const FOLDER_ID: [u8; 8] = [0x5c, 0xe4, 0x9e, 0x23, 0x18, 0x63, 0x7d, 0x10];
const BOLT_ID: [u8; 8] = [0x3a, 0xbc, 0xdc, 0x0f, 0x75, 0x2f, 0x7b, 0x0a];
const NO_SECTION: u32 = u32::MAX;

/// One directory record without the tree index that precedes it.
fn directory_record(
    path: &str,
    reference: [u8; 8],
    id: [u8; 8],
    stream: &str,
    section: u32,
    bodies: u32,
) -> Vec<u8> {
    let mut out = utf16_frame(path);
    out.extend(1234_u32.to_le_bytes());
    out.extend([0; 8]);
    out.extend(reference);
    out.extend(id);
    out.extend(utf16_frame(stream));
    out.extend(section.to_le_bytes());
    out.extend(bodies.to_le_bytes());
    for body in 0..bodies {
        for value in [101 + body, 80, 6] {
            out.extend(value.to_le_bytes());
        }
    }
    out
}

/// Header, then records joined by their tree indices (sub-assembly `Sub-1` takes index 4).
fn face_directory() -> Vec<u8> {
    let records = [
        (
            1_u32,
            directory_record("Plate-1@Asm", [0; 8], PLATE_ID, "000-000-001", 0, 1),
        ),
        (
            2,
            directory_record("Plate-2@Asm", PLATE_ID, PLATE_REPEAT_ID, "", NO_SECTION, 1),
        ),
        (
            3,
            directory_record("Folder1-11@Asm", [0; 8], FOLDER_ID, "", NO_SECTION, 0),
        ),
        (
            5,
            directory_record("Sub-1@Asm/Bolt-1@Sub", [0; 8], BOLT_ID, "000-000-001", 1, 1),
        ),
    ];
    let mut out = Vec::new();
    for value in [2_u32, 1, 4, records[0].0] {
        out.extend(value.to_le_bytes());
    }
    for (index, (tree_index, record)) in records.iter().enumerate() {
        if index > 0 {
            out.extend(tree_index.to_le_bytes());
        }
        out.extend(record);
    }
    out
}

fn extended_face_header(triangles: u32, strips: u32) -> Vec<u8> {
    let mut out = face_header(triangles, strips);
    for value in [1_u32, 0, 0, 0x20_0996, 0, 0, 0, 0] {
        out.extend(value.to_le_bytes());
    }
    out
}

/// Bounds and flags of a face trailer, then a persistent-surface reference.
fn face_trailer(reference: &str) -> Vec<u8> {
    let mut out = vec![0; 8];
    out.extend(1_u32.to_le_bytes());
    out.extend([0; 80]);
    out.extend(utf16_frame(reference));
    out
}

/// Two sections: the plate (two faces, one with an extended header) and the bolt.
fn face_stream() -> Vec<u8> {
    [
        0_u32.to_le_bytes().to_vec(),
        PLATE_ID.to_vec(),
        2_u32.to_le_bytes().to_vec(),
        face_header(2, 1),
        table(&QUAD, true),
        face_trailer("moSimpleSurfIdRep_c,7,1, "),
        extended_face_header(1, 1),
        table(&TRIANGLE, false),
        face_trailer("moEndFace3IntSurfIdRep_c,9,0,0,0, "),
        99_u32.to_le_bytes().to_vec(),
        1_u32.to_le_bytes().to_vec(),
        BOLT_ID.to_vec(),
        1_u32.to_le_bytes().to_vec(),
        face_header(1, 1),
        table(&TRIANGLE, true),
        face_trailer("moSimpleSurfIdRep_c,3,2, "),
        5_u32.to_le_bytes().to_vec(),
    ]
    .concat()
}

#[test]
fn face_tessellations_directory_lists_owners_repeats_and_folders() {
    let directory = face_tessellations::parse_directory(&face_directory())
        .unwrap_or_else(|| unreachable!("directory parses"));
    assert_eq!(directory.declared_records, 4);
    assert!(!directory.truncated);
    let records = &directory.records;
    assert_eq!(records.len(), 4);
    assert_eq!(
        records
            .iter()
            .map(|record| record.tree_index)
            .collect::<Vec<_>>(),
        [1, 2, 3, 5]
    );
    assert!(records[0].owns_tables() && !records[0].repeats());
    assert_eq!(records[0].section, Some(0));
    assert_eq!(records[0].stream.as_deref(), Some("000-000-001"));
    assert!(records[1].repeats() && !records[1].owns_tables());
    assert_eq!(records[1].reference_id, PLATE_ID);
    assert_eq!(records[1].leaf(), ("Plate".to_owned(), Some(2)));
    assert_eq!(records[2].body_count, 0);
    assert_eq!(records[3].leaf(), ("Bolt".to_owned(), Some(1)));
    assert_eq!(records[3].section, Some(1));
}

#[test]
fn face_tessellations_sections_attribute_validated_faces() {
    let directory = face_tessellations::parse_directory(&face_directory())
        .unwrap_or_else(|| unreachable!("directory parses"));
    let stream = face_stream();
    let (sections, missing) =
        face_tessellations::sections(&stream, &directory.records, "000-000-001");
    assert!(missing.is_empty());
    assert_eq!(sections.len(), 2);
    assert_eq!((sections[0].record, sections[0].declared_faces), (0, 2));
    assert_eq!((sections[1].record, sections[1].declared_faces), (3, 1));
    assert_eq!(sections[0].start, 4);
    // The trailing word and the next ordinal sit inside the previous range.
    assert_eq!(sections[0].end, sections[1].start);
    assert_eq!(sections[1].end, stream.len());
    let plate = face_tessellations::faces(&stream, sections[0].start + 12, sections[0].end);
    assert_eq!(plate.len(), 2);
    assert_eq!(plate[0].table.positions.len(), 4);
    assert_eq!(plate[0].surface_references[0].feature_source_id, 7);
    assert!(plate[1].table.normals.is_empty());
    assert_eq!(plate[1].surface_references[0].feature_source_id, 9);
    let bolt = face_tessellations::faces(&stream, sections[1].start + 12, sections[1].end);
    assert_eq!(bolt.len(), 1);
    assert_eq!(bolt[0].surface_references[0].local_surface_id, 2);
    // A table without a matching face header is not a face.
    let bare = table(&QUAD, true);
    assert!(face_tessellations::faces(&bare, 0, bare.len()).is_empty());
}

#[test]
fn face_tessellations_assembly_decodes_with_instances() -> Result<(), Box<dyn std::error::Error>> {
    let features = br#"<root><swHeader><swFile id="0" swDocType="ASSEMBLY"/></swHeader><swModelList><swModel id="self" swFileRef="0" swConfigurationId="0"><swConfiguration swID="0" swName="Default"/></swModel></swModelList></root>"#;
    let directory = face_directory();
    let stream = face_stream();
    let environment = b"no tables here".repeat(8);
    let input = crate::tests::modern_file(&[
        ("swXmlContents/Features", features),
        ("Contents/DisplayLists", &environment),
        ("FaceTessellations/Directory", &directory),
        ("FaceTessellations/000-000-001", &stream),
    ])?;
    let limits = sldkit_core::ResourceLimits::desktop();
    let (result, buffers) = crate::decode_display_mesh_bytes(&input, Some("asm.SLDASM"), &limits);
    assert_eq!(result.status, sldkit_core::DisplayMeshStatus::Decoded);
    let mesh = result.mesh.ok_or("mesh missing")?;
    assert_eq!(mesh.document_kind, sldkit_core::DocumentKind::Assembly);
    assert_eq!(mesh.streams.len(), 2);
    assert_eq!(mesh.streams[0].stream_path, "Contents/DisplayLists");
    assert_eq!(mesh.streams[0].part_count, 0);
    assert_eq!(mesh.streams[1].stream_path, "FaceTessellations/000-000-001");
    assert_eq!(mesh.streams[1].part_count, 3);
    assert_eq!(mesh.triangle_count, 4);
    assert_eq!(buffers.indices.len(), 12);
    assert_eq!(
        mesh.parts
            .iter()
            .map(|part| (part.stream_index, part.instance_index))
            .collect::<Vec<_>>(),
        [(1, Some(0)), (1, Some(0)), (1, Some(3))]
    );
    assert!(mesh.parts[1].normals_derived);
    assert_eq!(mesh.instances.len(), 4);
    let plate = &mesh.instances[0];
    assert_eq!((plate.name.as_str(), plate.number), ("Plate", Some(1)));
    assert_eq!(plate.tables_from, Some(0));
    assert_eq!(
        (plate.stream_index, plate.section_index),
        (Some(1), Some(0))
    );
    assert_eq!((plate.declared_face_count, plate.face_count), (Some(2), 2));
    let repeat = &mesh.instances[1];
    assert_eq!(repeat.tables_from, Some(0));
    assert_eq!(repeat.stream_index, None);
    assert_eq!(mesh.instances[2].tables_from, None);
    let bolt = &mesh.instances[3];
    assert_eq!(bolt.path, "Sub-1@Asm/Bolt-1@Sub");
    assert_eq!(bolt.tree_index, 5);
    assert_eq!(bolt.tables_from, Some(3));
    let codes = result
        .diagnostics
        .iter()
        .map(|item| item.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"display.face_tessellations_directory"));
    assert!(codes.contains(&"display.assembly_placement_unverified"));
    assert!(!codes.contains(&"display.stream_unreadable"));
    assert!(!codes.contains(&"display.multiple_streams"));
    Ok(())
}

#[test]
fn face_tessellations_without_directory_are_partial() -> Result<(), Box<dyn std::error::Error>> {
    let stream = face_stream();
    let input = crate::tests::modern_file(&[("FaceTessellations/000-000-001", &stream)])?;
    let limits = sldkit_core::ResourceLimits::desktop();
    let (result, _) = crate::decode_display_mesh_bytes(&input, Some("asm.SLDASM"), &limits);
    assert_eq!(result.status, sldkit_core::DisplayMeshStatus::Partial);
    let mesh = result.mesh.ok_or("mesh missing")?;
    assert!(mesh.instances.is_empty());
    assert_eq!(mesh.parts.len(), 3);
    assert!(mesh.parts.iter().all(|part| part.instance_index.is_none()));
    let codes = result
        .diagnostics
        .iter()
        .map(|item| item.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"display.face_tessellations_directory_unreadable"));
    assert!(codes.contains(&"display.face_tessellations_tables_unassigned"));
    Ok(())
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
