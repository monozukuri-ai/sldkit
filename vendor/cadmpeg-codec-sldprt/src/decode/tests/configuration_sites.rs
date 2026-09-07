// SPDX-License-Identifier: Apache-2.0
// Modified by sldkit; see the crate-root PATCHES.md.
//! Configuration site selection and partition-synthesis decode tests.
#![allow(clippy::unwrap_used)]
#![allow(unused_imports)]

use std::io::Cursor;

use cadmpeg_ir::codec::{Codec, DecodeOptions, Encoder};

use crate::container;
use crate::test_support::*;
use crate::SldprtCodec;

// sldkit patch: native configuration IDs are identities, not XML positions.
#[test]
fn native_configuration_ids_bind_sparse_partitions_and_saved_active_state() {
    let mut source = outer_header();
    for index in [7, 2] {
        source.extend(make_block(
            0x20,
            &format!("Contents/Config-{index}-Partition"),
            &parasolid_with_body("partition body", "SCH_SW_33103_11000", &triangle_body()),
        ));
    }
    source.extend(make_block(
        0x42,
        "Contents/Keywords",
        br#"<Keywords>
        <Configuration id="7" Name="Derived" Type="ConfigurationManager"/>
        <Configuration id="2" Name="Base" Type="ConfigurationManager"/>
        <Configuration id="9" Name="Missing" Type="ConfigurationManager"/>
        </Keywords>"#,
    ));
    source.extend(make_block(
        0x43,
        "Contents/SolidWorks",
        br#"<?xml version="1.0"?><swSolidWorks>
        <swModel swConfigurationName="Base"/>
        <swModel swConfigurationName="Derived"/>
        <swConfiguration swID="2" swMostRecentConfiguration="NO"/>
        <swConfiguration swID="7" swMostRecentConfiguration="YES"/>
        </swSolidWorks>"#,
    ));
    let decoded = SldprtCodec
        .decode(&mut Cursor::new(source), &DecodeOptions::default())
        .unwrap();
    let configurations = &decoded.ir().model.configurations;
    assert_eq!(configurations.len(), 3);
    let derived = configurations.iter().find(|c| c.name == "Derived").unwrap();
    let base = configurations.iter().find(|c| c.name == "Base").unwrap();
    let missing = configurations.iter().find(|c| c.name == "Missing").unwrap();
    assert_eq!(derived.source_index, Some(7));
    assert_eq!(base.source_index, Some(2));
    assert!(derived.active.is_active());
    assert!(base.active.is_inactive());
    assert!(missing.bodies.is_unresolved());
    assert_eq!(derived.bodies.resolved().unwrap().len(), 1);
    assert_eq!(base.bodies.resolved().unwrap().len(), 1);
    assert_ne!(derived.bodies, base.bodies);
}

#[test]
fn malformed_or_conflicting_native_ids_never_bind_by_position_or_name() {
    for attributes in [
        "id=\"invalid\" Type=\"ConfigurationManager\"",
        "id=\"0\" SourceIndex=\"1\" Type=\"ConfigurationManager\"",
        "id=\"0\" SourceIndex=\"invalid\" Type=\"ConfigurationManager\"",
        "id=\"0\" Type=\"UnknownManager\"",
    ] {
        let mut source = sldprt_with_body(&triangle_body());
        source.extend(make_block(
            0x42,
            "Contents/Keywords",
            format!("<Keywords><Configuration Name=\"Base\" {attributes}/></Keywords>").as_bytes(),
        ));
        let decoded = SldprtCodec
            .decode(&mut Cursor::new(source), &DecodeOptions::default())
            .unwrap();
        let native = decoded
            .ir()
            .model
            .configurations
            .iter()
            .find(|c| c.name == "Base")
            .unwrap();
        assert_eq!(native.source_index, None, "{attributes}");
        assert!(native.bodies.is_unresolved(), "{attributes}");
    }
}

#[test]
fn contradictory_saved_active_flags_do_not_select_first_xml_model() {
    let mut source = sldprt_with_body(&triangle_body());
    source.extend(make_block(
        0x42,
        "Contents/Keywords",
        br#"<Keywords>
        <Configuration id="0" Name="Base" Type="ConfigurationManager"/>
        <Configuration id="1" Name="Derived" Type="ConfigurationManager"/>
        </Keywords>"#,
    ));
    source.extend(make_block(
        0x43,
        "Contents/SolidWorks",
        br#"<?xml version="1.0"?><swSolidWorks>
        <swModel swConfigurationName="Base"/>
        <swConfiguration swID="0" swMostRecentConfiguration="YES"/>
        <swConfiguration swID="1" swMostRecentConfiguration="YES"/>
        </swSolidWorks>"#,
    ));
    let decoded = SldprtCodec
        .decode(&mut Cursor::new(source), &DecodeOptions::default())
        .unwrap();
    assert!(decoded
        .ir()
        .model
        .configurations
        .iter()
        .all(|c| c.active.is_inactive()));
}

#[test]
fn decode_preserves_unresolved_active_configuration() {
    let mut source = sldprt_with_body(&triangle_body());
    source.extend(make_block(
        0x42,
        "Contents/Keywords",
        br#"<Keywords><Configuration Name="Default"/><Configuration Name="Manufacturing"/></Keywords>"#,
    ));
    source.extend(make_block(
        0x43,
        "Contents/SolidWorks",
        br#"<?xml version="1.0"?><swSolidWorks swVersion="34000"><swModel swName="Part" swConfigurationName="Missing"/></swSolidWorks>"#,
    ));
    assert_eq!(
        container::active_configuration_index(&container::scan_bytes(&source)),
        None
    );

    let decoded = SldprtCodec
        .decode(&mut Cursor::new(source), &DecodeOptions::default())
        .unwrap();

    assert!(decoded
        .ir()
        .model
        .configurations
        .iter()
        .all(|configuration| configuration.active.is_inactive()));
    assert!(decoded.report().losses.iter().any(|loss| {
        loss.message
            == "active configuration identity is unresolved; 0 of 3 configuration records are active."
    }));
    assert!(cadmpeg_ir::validate_neutral(decoded.ir(), Vec::new()).is_ok());
}

#[test]
fn decode_reports_partition_inferred_configuration() {
    let decoded = SldprtCodec
        .decode(
            &mut Cursor::new(sldprt_with_body(&triangle_body())),
            &DecodeOptions::default(),
        )
        .unwrap();

    assert_eq!(decoded.ir().model.configurations.len(), 1);
    assert!(decoded.ir().model.configurations[0].native_ref.is_none());
    assert!(decoded.report().losses.iter().any(|loss| {
        loss.message
            == "1 configuration state(s) are inferred from geometry partitions without native configuration definitions."
    }));
}

#[test]
fn decode_assigns_selected_partition_bodies_to_configuration() {
    let mut source = sldprt_with_body(&triangle_body());
    source.extend(make_block(
        0x42,
        "Contents/Keywords",
        br#"<Keywords><Configuration Name="Default" SourceIndex="0"/></Keywords>"#,
    ));
    let decoded = SldprtCodec
        .decode(&mut Cursor::new(source), &DecodeOptions::default())
        .unwrap();
    assert_eq!(decoded.ir().model.configurations.len(), 1);
    assert!(decoded.ir().model.configurations[0].active.is_active());
    assert_eq!(
        decoded.ir().model.configurations[0].bodies,
        decoded
            .ir()
            .model
            .bodies
            .iter()
            .map(|body| body.id.clone())
            .collect::<Vec<_>>()
    );
    let mut written = Vec::new();
    SldprtCodec
        .write_preserved_with_source_fidelity(decoded.ir(), decoded.source_fidelity(), &mut written)
        .unwrap();
    let round_trip = SldprtCodec
        .decode(&mut Cursor::new(written), &DecodeOptions::default())
        .unwrap();
    assert_eq!(
        round_trip.ir().model.configurations[0].bodies,
        round_trip
            .ir()
            .model
            .bodies
            .iter()
            .map(|body| body.id.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn decode_synthesizes_sparse_partition_configuration() {
    let mut source = outer_header();
    source.extend(make_block(
        0x20,
        "Contents/Config-3-Partition",
        &parasolid_with_body("partition body", "SCH_SW_33103_11000", &triangle_body()),
    ));
    assert_eq!(
        container::scan_bytes(&source).blocks[0].section.as_deref(),
        Some("Contents/Config-3-Partition")
    );
    let decoded = SldprtCodec
        .decode(&mut Cursor::new(source), &DecodeOptions::default())
        .unwrap();
    assert_eq!(decoded.ir().model.configurations.len(), 1);
    let configuration = &decoded.ir().model.configurations[0];
    assert_eq!(configuration.ordinal, 0);
    assert_eq!(configuration.source_index, Some(3));
    assert!(configuration.active.is_active());
    assert_eq!(configuration.name, "Config-3");
    assert_eq!(
        configuration.bodies,
        decoded
            .ir()
            .model
            .bodies
            .iter()
            .map(|body| body.id.clone())
            .collect::<Vec<_>>()
    );

    let (mut edited, _, fidelity) = decoded.into_parts();
    edited.model.points[0].position.x += 1.0;
    let mut written = Vec::new();
    SldprtCodec
        .write_preserved_with_source_fidelity(&edited, &fidelity, &mut written)
        .unwrap();
    let scan = container::scan_bytes(&written);
    assert!(scan
        .blocks
        .iter()
        .any(|block| block.section.as_deref() == Some("Contents/Config-3-Partition")));
    assert!(!scan
        .blocks
        .iter()
        .any(|block| block.section.as_deref() == Some("Contents/Config-0-Partition")));
}

#[test]
fn decode_merges_colliding_configuration_sites_with_disjoint_identities() {
    let mut cur = Cursor::new(sldprt_with_colliding_sites());
    let result = SldprtCodec
        .decode(&mut cur, &DecodeOptions::default())
        .unwrap();
    assert_eq!(result.ir().model.faces.len(), 2);
    assert!(result
        .ir()
        .model
        .points
        .iter()
        .any(|point| point.position.x == 0.0));
    assert!(result
        .ir()
        .model
        .points
        .iter()
        .any(|point| point.position.x == 10_000.0));
    let ids: std::collections::HashSet<_> = result
        .ir()
        .model
        .points
        .iter()
        .map(|point| &point.id)
        .collect();
    assert_eq!(ids.len(), result.ir().model.points.len());
    assert!(result
        .ir()
        .model
        .points
        .iter()
        .all(|point| point.id.0.contains("@block@")));
    let report = cadmpeg_ir::validate::validate_neutral(result.ir(), Vec::new());
    assert!(report.is_ok(), "validation findings: {:?}", report.findings);
}

#[test]
fn decode_uses_the_active_configuration_source_site() {
    let mut source = sldprt_with_colliding_sites();
    source.extend(make_block(
        0x42,
        "Contents/Keywords",
        br#"<Keywords><Configuration Name="First" SourceIndex="0"/><Configuration Name="Second" SourceIndex="1"/></Keywords>"#,
    ));
    source.extend(make_block(
        0x43,
        "Contents/SolidWorks",
        br#"<?xml version="1.0"?><swSolidWorks><swModel swConfigurationName="Second"/></swSolidWorks>"#,
    ));

    let result = SldprtCodec
        .decode(&mut Cursor::new(source), &DecodeOptions::default())
        .unwrap();

    let active_points = result
        .ir()
        .model
        .points
        .iter()
        .filter(|point| !point.id.0.contains("@block@"))
        .collect::<Vec<_>>();
    assert_eq!(active_points.len(), 3);
    assert!(active_points
        .iter()
        .all(|point| point.position.x >= 10_000.0));
    assert_eq!(
        result.ir().source.as_ref().unwrap().attributes["active_parasolid_block"],
        "Contents/Config-1-Partition"
    );
    assert!(cadmpeg_ir::validate_neutral(result.ir(), result.report().losses.clone()).is_ok());
}
