// SPDX-License-Identifier: Apache-2.0
// Modified by sldkit; see the crate-root PATCHES.md.
#![allow(clippy::unwrap_used)]

use super::*;
use cadmpeg_ir::features::{ConfigurationBodies, DesignConfiguration};
use cadmpeg_ir::ids::{BodyId, FaceId, RegionId, ShellId};
use cadmpeg_ir::tessellation::Tessellation;
use cadmpeg_ir::ConfigurationId;
use std::collections::BTreeMap;

fn config(name: &str, body: BodyId) -> DesignConfiguration {
    DesignConfiguration {
        id: ConfigurationId(name.into()),
        ordinal: 0,
        active: false.into(),
        source_index: None,
        name: name.into(),
        material: None,
        properties: BTreeMap::default(),
        bodies: ConfigurationBodies::Resolved(vec![body]),
        parameter_values: BTreeMap::default(),
        suppressed_features: Vec::new(),
        parameter_overrides: BTreeMap::default(),
        feature_states: BTreeMap::default(),
        native_ref: None,
    }
}

fn reference(feature: u32) -> PersistentSurfaceReference {
    PersistentSurfaceReference {
        identity: format!("moFromSktEntSurfIdRep_c,{feature},1,"),
        feature_source_id: feature,
        local_surface_id: 1,
    }
}

fn fixture() -> (Model, DisplayOwnership, Vec<(String, u32, u32)>) {
    let mut model = crate::test_support::source_less_cube().model;
    let base = model.bodies[0].id.clone();
    let mut body = model.bodies[0].clone();
    body.id = BodyId("derived-body".into());
    body.regions = vec![RegionId("derived-region".into())];
    let derived = body.id.clone();
    let mut region = model.regions[0].clone();
    region.id = body.regions[0].clone();
    region.body = body.id.clone();
    region.shells = vec![ShellId("derived-shell".into())];
    let mut shell = model.shells[0].clone();
    shell.id = region.shells[0].clone();
    shell.region = region.id.clone();
    let base_face = model.faces[0].id.0.clone();
    let mut shared = model.faces[0].clone();
    shared.id = FaceId("shared-derived".into());
    shared.shell = shell.id.clone();
    let mut extra = model.faces[1].clone();
    extra.id = FaceId("extra-derived".into());
    extra.shell = shell.id.clone();
    shell.faces = vec![shared.id.clone(), extra.id.clone()];
    model.bodies.push(body);
    model.regions.push(region);
    model.shells.push(shell);
    model.faces.extend([shared, extra]);
    model.configurations = vec![config("Base", base), config("Derived", derived)];
    for id in ["shared", "extra"] {
        model.tessellations.push(Tessellation {
            id: id.into(),
            body: None,
            faces: Vec::new(),
            chordal_deflection: None,
            source_object: None,
            vertices: Vec::new(),
            triangles: Vec::new(),
            feature_edges: Vec::new(),
            strip_lengths: Vec::new(),
            normals: Vec::new(),
            corner_normals: Vec::new(),
            triangle_groups: Vec::new(),
            texture_assignments: Vec::new(),
            channels: Vec::new(),
        });
    }
    let source = DisplayOwnership {
        references: HashMap::from([
            ("shared".into(), (10, vec![reference(30), reference(30)])),
            ("extra".into(), (10, vec![reference(37)])),
        ]),
        configuration_names: HashMap::from([(10, HashSet::from(["Derived".into()]))]),
    };
    let identities = vec![
        (base_face, 30, 1),
        ("shared-derived".into(), 30, 1),
        ("extra-derived".into(), 37, 1),
    ];
    (model, source, identities)
}

#[test]
fn all_cache_references_and_saved_name_jointly_select_configuration() {
    let (model, source, identities) = fixture();
    let constraints = face_constraints(&model, &source, &identities);
    assert_eq!(
        constraints["shared"],
        HashSet::from(["shared-derived".into()])
    );
    assert_eq!(
        constraints["extra"],
        HashSet::from(["extra-derived".into()])
    );
}

#[test]
fn missing_or_conflicting_names_do_not_resolve_shared_faces() {
    for names in [vec![], vec!["Base"], vec!["Base", "Derived"]] {
        let (model, mut source, identities) = fixture();
        source
            .configuration_names
            .insert(10, names.into_iter().map(String::from).collect());
        assert_eq!(
            face_constraints(&model, &source, &identities)["shared"].len(),
            2
        );
    }
}

#[test]
fn saved_name_cannot_choose_an_ambiguous_configuration() {
    let (mut model, source, identities) = fixture();
    model.tessellations.retain(|mesh| mesh.id == "shared");
    assert_eq!(
        face_constraints(&model, &source, &identities)["shared"].len(),
        2
    );
}

#[test]
fn distinct_streams_or_unresolved_membership_do_not_supply_configuration_scope() {
    let (mut model, mut source, identities) = fixture();
    source.references.get_mut("extra").unwrap().0 = 11;
    assert_eq!(
        face_constraints(&model, &source, &identities)["shared"].len(),
        2
    );
    source.references.get_mut("extra").unwrap().0 = 10;
    model.configurations[0].bodies = ConfigurationBodies::Unresolved;
    assert_eq!(
        face_constraints(&model, &source, &identities)["shared"].len(),
        2
    );
}

#[test]
fn conflicting_or_unknown_persistent_references_block_geometric_fallback() {
    let (model, mut source, identities) = fixture();
    source.references.get_mut("shared").unwrap().1[1].identity =
        "moEndFaceSurfIdRep_c,30,1,0,".into();
    assert!(face_constraints(&model, &source, &identities)["shared"].is_empty());
    source.references.get_mut("shared").unwrap().1 = vec![reference(999)];
    assert!(face_constraints(&model, &source, &identities)["shared"].is_empty());
    source.references.get_mut("shared").unwrap().1.clear();
    assert!(!face_constraints(&model, &source, &identities).contains_key("shared"));
}
