// SPDX-License-Identifier: Apache-2.0
// Modified by sldkit; see the crate-root PATCHES.md.
//! Display cache transfer is independent of B-Rep availability.
#![allow(clippy::unwrap_used)]

use std::io::Cursor;

use cadmpeg_ir::codec::{Codec, DecodeOptions};

use crate::test_support::*;
use crate::{container, SldprtCodec};

fn with_display(mut source: Vec<u8>, payload: &[u8]) -> Vec<u8> {
    source.extend(make_block(0x41, "Contents/DisplayLists", payload));
    source
}

#[test]
fn transfers_display_once_with_absent_unsupported_or_decoded_brep() {
    let payload = display_list_payload();
    for (source, has_brep) in [
        (outer_header(), false),
        (synthetic_sldprt(), false),
        (sldprt_with_body(&triangle_body()), true),
    ] {
        let source = with_display(source, &payload);
        let decoded = SldprtCodec
            .decode(&mut Cursor::new(&source), &DecodeOptions::default())
            .unwrap();
        assert_eq!(decoded.report().geometry_transferred, has_brep);
        let model = &decoded.ir().model;
        assert_eq!(model.tessellations.len(), 1);
        let mesh = &model.tessellations[0];
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.vertices[1].x, 1000.0);
        assert_eq!(mesh.triangles, [[0, 1, 2]]);
        assert_eq!(mesh.strip_lengths, [3]);
        assert_eq!(mesh.channels.len(), 6);
        assert_eq!(
            decoded.source_fidelity().annotations.provenance[&mesh.id]
                .tag
                .as_deref(),
            Some("displaylist_tessellation")
        );
        assert!(decoded
            .source_fidelity()
            .retained_records
            .iter()
            .any(|record| { record.sha256 == cadmpeg_ir::hash::sha256_hex(&payload) }));
        if !has_brep {
            assert!(model.bodies.is_empty());
            assert!(model.faces.is_empty());
            assert!(mesh.body.is_none());
            assert!(mesh.faces.is_empty());
            assert!(decoded.report().losses.iter().any(|loss| loss
                .message
                .contains("do not resolve to B-rep face ownership")));
        }
        let mut replay = Vec::new();
        SldprtCodec
            .write_preserved_with_source_fidelity(
                decoded.ir(),
                decoded.source_fidelity(),
                &mut replay,
            )
            .unwrap();
        assert_eq!(replay, source);
    }
}

#[test]
fn absent_or_truncated_cache_does_not_create_meshes() {
    let payload = display_list_payload();
    for source in [
        synthetic_sldprt(),
        with_display(synthetic_sldprt(), &payload[..payload.len() / 2]),
    ] {
        let decoded = SldprtCodec
            .decode(&mut Cursor::new(source), &DecodeOptions::default())
            .unwrap();
        assert!(!decoded.report().geometry_transferred);
        assert!(decoded.ir().model.tessellations.is_empty());
    }
}

#[test]
fn container_only_skips_meshes_and_strict_still_rejects_missing_brep() {
    let source = with_display(outer_header(), &display_list_payload());
    let options = DecodeOptions {
        container_only: true,
        ..strict_options()
    };
    let decoded = SldprtCodec
        .decode(&mut Cursor::new(&source), &options)
        .unwrap();
    assert!(decoded.report().container_only);
    assert!(decoded.ir().model.tessellations.is_empty());
    assert!(SldprtCodec
        .decode(&mut Cursor::new(source), &strict_options())
        .is_err());
}

#[test]
fn fallback_display_entities_are_charged_additively() {
    use cadmpeg_core::decode::ResourceDimension;

    let source = with_display(outer_header(), &display_list_payload());
    let scan = container::scan_bytes(&source);
    let container_count = scan.blocks.len() + scan.compound_streams.len() + scan.directory.len();
    let decoded = SldprtCodec
        .decode(&mut Cursor::new(&source), &DecodeOptions::default())
        .unwrap();
    let total = (container_count + decoded.ir().model.entity_count()) as u64;
    let mut options = DecodeOptions::default();
    options.policy.limits.max_entities = total - 1;
    let error = SldprtCodec
        .decode(&mut Cursor::new(&source), &options)
        .unwrap_err();
    assert!(
        matches!(error, cadmpeg_core::CodecError::ResourceLimit(limit)
        if limit.dimension == ResourceDimension::Entities)
    );
    options.policy.limits.max_entities = total;
    SldprtCodec
        .decode(&mut Cursor::new(source), &options)
        .unwrap();
}

#[test]
fn fallback_display_is_identical_with_the_byte_ledger() {
    let source = with_display(synthetic_sldprt(), &display_list_payload());
    let options = DecodeOptions::default();
    let ordinary = SldprtCodec
        .decode(&mut Cursor::new(&source), &options)
        .unwrap();
    let (instrumented, ledger) = SldprtCodec
        .decode_with_byte_ledger(&mut Cursor::new(&source), &options)
        .unwrap();
    assert_eq!(
        serde_json::to_value(ordinary.ir()).unwrap(),
        serde_json::to_value(instrumented.ir()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(ordinary.report()).unwrap(),
        serde_json::to_value(instrumented.report()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(ordinary.source_fidelity()).unwrap(),
        serde_json::to_value(instrumented.source_fidelity()).unwrap()
    );
    assert!(ledger.domains.is_empty());
}
