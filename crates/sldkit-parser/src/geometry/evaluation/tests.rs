use std::{error::Error, path::PathBuf};

use cadmpeg_codec_sldprt::SldprtCodec;
use cadmpeg_ir::{Encoder, codec::EncodeInput};
use sldkit_core::{
    BoundingBoxMethod, EdgeIntervalSource, EdgeSampleStatus, EdgeSmoothness,
    GeometryConfigurationState, GeometryDocument, GeometryEvaluationOptions,
    GeometryEvaluationStatus, ResourceLimits,
};

use super::{evaluate, mesh::tests::cube_tessellation};

fn decoded_cube() -> Result<GeometryDocument, Box<dyn Error>> {
    let mut ir = cadmpeg_ir::examples::unit_cube();
    ir.model.bodies[0].name = None;
    for face in &mut ir.model.faces {
        face.name = None;
    }
    for edge in &mut ir.model.edges {
        edge.param_range = None;
    }
    let mut bytes = Vec::new();
    SldprtCodec
        .plan(EncodeInput {
            ir: &ir,
            fidelity: None,
        })?
        .write_to(&mut bytes)?;
    let result = crate::decode_geometry_bytes(&bytes, None, &ResourceLimits::service());
    result
        .geometry
        .ok_or_else(|| "synthetic cube did not decode".into())
}

fn corpus_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn invalid_options_are_rejected_before_any_evaluation() -> Result<(), Box<dyn Error>> {
    let document = decoded_cube()?;
    let cases: [fn(&mut GeometryEvaluationOptions); 5] = [
        |o| o.chord_tolerance_mm = 0.0,
        |o| o.chord_tolerance_mm = f64::NAN,
        |o| o.max_segments_per_edge = 0,
        |o| o.tangent_angle_deg = -1.0,
        |o| o.mesh_feature_angle_deg = 181.0,
    ];
    for (index, mutate) in cases.into_iter().enumerate() {
        let mut options = GeometryEvaluationOptions::default();
        mutate(&mut options);
        let result = evaluate(&document, &options);
        assert_eq!(
            result.status,
            GeometryEvaluationStatus::Rejected,
            "case {index}"
        );
        assert!(result.evaluation.is_none());
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(
            result.diagnostics[0].code,
            "geometry.evaluation_options_invalid"
        );
    }
    Ok(())
}

// Exact bounds of an axis-aligned cube are integral, so strict comparison is intended.
#[test]
#[allow(clippy::float_cmp)]
fn synthetic_cube_is_fully_evaluated_with_exact_bounds() -> Result<(), Box<dyn Error>> {
    let document = decoded_cube()?;
    let options = GeometryEvaluationOptions::default();
    let result = evaluate(&document, &options);
    assert_eq!(result.status, GeometryEvaluationStatus::Evaluated);
    assert!(result.diagnostics.is_empty());
    let evaluation = result.evaluation.ok_or("evaluation missing")?;
    assert!(evaluation.losses.is_empty());
    assert_eq!(evaluation.edges.len(), 12);
    assert_eq!(evaluation.summary.sampled_edge_count, 12);
    assert_eq!(evaluation.summary.sharp_edge_count, 12);
    assert_eq!(evaluation.summary.unknown_smoothness_count, 0);
    for edge in &evaluation.edges {
        assert_eq!(edge.status, EdgeSampleStatus::Sampled);
        assert_eq!(edge.interval_source, EdgeIntervalSource::Derived);
        assert_eq!(edge.curve_kind, "line");
        assert_eq!(edge.polyline.len(), 2);
        assert_eq!(edge.segment_count, 1);
        assert_eq!(edge.adjacent_face_ids.len(), 2);
        assert_eq!(edge.smoothness, EdgeSmoothness::Sharp);
        let angle = edge.dihedral_angle_deg.ok_or("dihedral missing")?;
        assert!((angle - 90.0).abs() < 1e-9, "{angle}");
        assert!(edge.body_id.is_some());
        assert!(!edge.is_derived_seam);
        assert_eq!(edge.chord_tolerance_mm, Some(0.0));
    }
    let bounding_box = evaluation
        .bounding_box
        .as_ref()
        .ok_or("bounding box missing")?;
    assert_eq!(bounding_box.method, BoundingBoxMethod::EdgeSamples);
    assert!(bounding_box.exact);
    assert_eq!(bounding_box.edge_count, 12);
    assert_eq!(bounding_box.bounds.min, [0.0; 3]);
    assert_eq!(bounding_box.bounds.max, [10.0; 3]);
    assert_eq!(evaluation.body_bounds.len(), 1);
    assert_eq!(
        evaluation.body_bounds[0]
            .bounding_box
            .as_ref()
            .map(|b| b.bounds),
        Some(bounding_box.bounds)
    );
    assert!(evaluation.meshes.is_empty());
    assert_eq!(evaluation.length_unit, document.length_unit);

    let repeated = evaluate(&document, &options);
    assert_eq!(repeated, result_like(&evaluation, &repeated));
    let json = serde_json::to_string(&evaluation)?;
    assert!(json.contains("\"interval_source\":\"derived\""));
    assert!(json.contains("\"smoothness\":\"sharp\""));
    Ok(())
}

fn result_like(
    expected: &sldkit_core::GeometryEvaluationDocument,
    actual: &sldkit_core::GeometryEvaluationResult,
) -> sldkit_core::GeometryEvaluationResult {
    sldkit_core::GeometryEvaluationResult {
        status: actual.status,
        evaluation: Some(expected.clone()),
        diagnostics: actual.diagnostics.clone(),
    }
}

#[test]
fn withheld_edges_make_the_result_partial_and_inexact() -> Result<(), Box<dyn Error>> {
    let mut document = decoded_cube()?;
    let mut edges = document.model.edges.iter_mut();
    let first = edges.next().ok_or("edge missing")?;
    first.curve_id = None;
    let second = edges.next().ok_or("edge missing")?;
    second.parameter_range = None;
    second.derived_parameter_interval = None;
    let result = evaluate(&document, &GeometryEvaluationOptions::default());
    assert_eq!(result.status, GeometryEvaluationStatus::Partial);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "geometry.evaluation_partial")
    );
    let evaluation = result.evaluation.ok_or("evaluation missing")?;
    assert_eq!(evaluation.summary.unsampled_edge_count, 2);
    assert_eq!(evaluation.summary.unknown_smoothness_count, 2);
    let reasons = evaluation
        .edges
        .iter()
        .filter_map(|edge| edge.reason.as_deref())
        .collect::<Vec<_>>();
    assert!(reasons.contains(&"curve_missing"));
    assert!(reasons.contains(&"interval_unavailable"));
    let codes = evaluation
        .losses
        .iter()
        .map(|loss| loss.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        vec![
            "evaluation.edge_curve_missing",
            "evaluation.edge_interval_unavailable"
        ]
    );
    assert!(evaluation.losses[0].message.starts_with("1 occurrence"));
    let bounding_box = evaluation.bounding_box.ok_or("bounding box missing")?;
    assert!(!bounding_box.exact);
    assert_eq!(bounding_box.edge_count, 10);
    Ok(())
}

// Exact bounds of an axis-aligned cube are integral, so strict comparison is intended.
#[test]
#[allow(clippy::float_cmp)]
fn meshes_add_fallback_quantities_and_keep_exact_edge_bounds() -> Result<(), Box<dyn Error>> {
    let mut document = decoded_cube()?;
    let body_id = document.model.bodies[0].id.clone();
    let mut owned = cube_tessellation("owned", 10.0);
    owned.body_id = Some(body_id.clone());
    let mut orphan = cube_tessellation("orphan", 5.0);
    let _ = orphan.triangles.pop();
    document.model.tessellations = vec![owned, orphan];
    let result = evaluate(&document, &GeometryEvaluationOptions::default());
    assert_eq!(result.status, GeometryEvaluationStatus::Evaluated);
    let evaluation = result.evaluation.ok_or("evaluation missing")?;
    assert_eq!(evaluation.meshes.len(), 2);
    assert_eq!(evaluation.summary.mesh_count, 2);
    assert_eq!(evaluation.summary.closed_mesh_count, 1);
    let owned = &evaluation.meshes[1];
    assert_eq!(owned.tessellation_id, "owned");
    assert_eq!(owned.is_closed, Some(true));
    assert!((owned.signed_volume_mm3.unwrap_or(0.0) - 1000.0).abs() < 1e-9);
    assert_eq!(owned.feature_edges.len(), 12);
    let orphan = &evaluation.meshes[0];
    assert_eq!(orphan.is_closed, Some(false));
    assert_eq!(orphan.boundary_edges.len(), 3);
    assert_eq!(
        evaluation
            .losses
            .iter()
            .map(|l| l.code.as_str())
            .collect::<Vec<_>>(),
        vec!["evaluation.mesh_not_closed"]
    );
    let bounding_box = evaluation.bounding_box.ok_or("bounding box missing")?;
    assert_eq!(
        bounding_box.method,
        BoundingBoxMethod::EdgeSamplesAndTessellation
    );
    assert!(bounding_box.exact);
    assert_eq!(bounding_box.vertex_count, 16);
    assert_eq!(bounding_box.bounds.max, [10.0; 3]);
    let body = evaluation.body_bounds[0]
        .bounding_box
        .as_ref()
        .ok_or("body bounds missing")?;
    assert_eq!(body.method, BoundingBoxMethod::EdgeSamplesAndTessellation);
    assert_eq!(body.vertex_count, 8);

    let mut transformed = decoded_cube()?;
    transformed.model.bodies[0].transform = Some(serde_json::json!({"translation": [1, 2, 3]}));
    let result = evaluate(&transformed, &GeometryEvaluationOptions::default());
    assert_eq!(result.status, GeometryEvaluationStatus::Partial);
    let evaluation = result.evaluation.ok_or("evaluation missing")?;
    assert!(
        evaluation
            .losses
            .iter()
            .any(|loss| loss.code == "evaluation.body_transform_not_applied")
    );
    assert!(!evaluation.bounding_box.ok_or("bounds")?.exact);
    Ok(())
}

#[test]
fn configuration_selection_filters_or_rejects_explicitly() -> Result<(), Box<dyn Error>> {
    let mut document = decoded_cube()?;
    let body_id = document.model.bodies[0].id.clone();
    document.configurations = vec![
        GeometryConfigurationState {
            id: "cfg:empty".to_owned(),
            ordinal: 0,
            active: Some(false),
            source_index: None,
            name: Some("Empty".to_owned()),
            body_ids: Some(Vec::new()),
        },
        GeometryConfigurationState {
            id: "cfg:full".to_owned(),
            ordinal: 1,
            active: Some(true),
            source_index: None,
            name: Some("Full".to_owned()),
            body_ids: Some(vec![body_id]),
        },
        GeometryConfigurationState {
            id: "cfg:unresolved".to_owned(),
            ordinal: 2,
            active: None,
            source_index: None,
            name: Some("Unresolved".to_owned()),
            body_ids: None,
        },
    ];

    let missing = evaluate(
        &document,
        &GeometryEvaluationOptions {
            configuration: Some("Nope".to_owned()),
            ..GeometryEvaluationOptions::default()
        },
    );
    assert_eq!(missing.status, GeometryEvaluationStatus::Rejected);
    assert_eq!(
        missing.diagnostics[0].code,
        "geometry.evaluation_configuration_missing"
    );

    let empty = evaluate(
        &document,
        &GeometryEvaluationOptions {
            configuration: Some("Empty".to_owned()),
            ..GeometryEvaluationOptions::default()
        },
    );
    assert_eq!(empty.status, GeometryEvaluationStatus::Evaluated);
    let evaluation = empty.evaluation.ok_or("evaluation missing")?;
    assert!(evaluation.edges.is_empty());
    assert!(evaluation.bounding_box.is_none());
    assert_eq!(evaluation.body_bounds.len(), 1);
    assert_eq!(evaluation.configuration_bounds.len(), 3);
    assert_eq!(
        evaluation.configuration_bounds[0].configuration_id,
        "cfg:empty"
    );
    assert!(evaluation.configuration_bounds[0].bounding_box.is_none());
    assert!(evaluation.configuration_bounds[1].bounding_box.is_some());
    assert!(evaluation.configuration_bounds[2].body_ids.is_none());
    assert!(evaluation.configuration_bounds[2].bounding_box.is_none());

    let full = evaluate(
        &document,
        &GeometryEvaluationOptions {
            configuration: Some("cfg:full".to_owned()),
            ..GeometryEvaluationOptions::default()
        },
    );
    assert_eq!(full.status, GeometryEvaluationStatus::Evaluated);
    assert_eq!(full.evaluation.ok_or("evaluation missing")?.edges.len(), 12);

    let unresolved = evaluate(
        &document,
        &GeometryEvaluationOptions {
            configuration: Some("Unresolved".to_owned()),
            ..GeometryEvaluationOptions::default()
        },
    );
    assert_eq!(unresolved.status, GeometryEvaluationStatus::Partial);
    assert!(
        unresolved
            .diagnostics
            .iter()
            .any(|d| d.code == "geometry.evaluation_configuration_unresolved")
    );
    assert_eq!(
        unresolved
            .evaluation
            .ok_or("evaluation missing")?
            .edges
            .len(),
        12
    );
    Ok(())
}

#[test]
fn bytes_entry_point_reports_unavailable_geometry() {
    let result = crate::evaluate_geometry_bytes(
        b"not a container",
        None,
        &ResourceLimits::service(),
        &GeometryEvaluationOptions::default(),
    );
    assert_eq!(result.status, GeometryEvaluationStatus::Unavailable);
    assert!(result.evaluation.is_none());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "geometry.evaluation_unavailable")
    );
    let rejected = crate::evaluate_geometry_path(
        corpus_path("corpus/does-not-exist.SLDPRT"),
        &ResourceLimits::service(),
        &GeometryEvaluationOptions::default(),
    );
    assert_eq!(rejected.status, GeometryEvaluationStatus::Rejected);
}

#[test]
fn local_m5_controlled_part_samples_every_edge() -> Result<(), Box<dyn Error>> {
    let path = corpus_path("corpus/external/local/m5-controlled/M5-Controlled.SLDPRT");
    if !path.exists() {
        return Ok(());
    }
    let result = crate::evaluate_geometry_path(
        &path,
        &ResourceLimits::service(),
        &GeometryEvaluationOptions::default(),
    );
    let evaluation = result.evaluation.ok_or("evaluation missing")?;
    assert_eq!(evaluation.summary.edge_count, 40);
    assert_eq!(evaluation.summary.unsampled_edge_count, 0);
    assert_eq!(evaluation.body_bounds.len(), 4);
    assert_eq!(evaluation.configuration_bounds.len(), 2);
    assert!(
        evaluation
            .configuration_bounds
            .iter()
            .all(|bounds| bounds.bounding_box.is_some())
    );
    let model = evaluation.bounding_box.ok_or("bounding box missing")?;
    assert!(
        !model.exact,
        "a NURBS face must not claim exact edge bounds"
    );
    assert_eq!(evaluation.summary.mesh_count, 13);
    Ok(())
}

#[test]
fn local_test3_part_matches_the_assembly_bounding_box() -> Result<(), Box<dyn Error>> {
    let archive = corpus_path("corpus/external/local/TEST3.zip");
    if !archive.exists() {
        return Ok(());
    }
    let data = std::fs::read(&archive)?;
    let limits = ResourceLimits::service();
    let inventory = crate::inspect_bytes(&data, Some("TEST3.zip"), &limits);
    let entry = inventory
        .inventory
        .as_ref()
        .and_then(|inventory| {
            inventory
                .entries
                .iter()
                .find(|entry| entry.path.as_deref() == Some("TEST1.SLDPRT"))
        })
        .ok_or("TEST1.SLDPRT entry missing")?;
    // The bounded ZIP reader keeps deflate payloads as stored bytes; the
    // fixture archive is inflated here, outside the parser's own contract.
    let extraction = crate::extract_bytes(
        &data,
        &entry.id,
        sldkit_core::ExtractionMode::Stored,
        &limits,
    );
    let stored = extraction
        .data
        .ok_or("TEST1.SLDPRT could not be extracted")?;
    let mut part = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::DeflateDecoder::new(stored.as_slice()),
        &mut part,
    )?;
    let result = crate::evaluate_geometry_bytes(
        &part,
        Some("TEST1.SLDPRT"),
        &limits,
        &GeometryEvaluationOptions::default(),
    );
    assert_eq!(result.status, GeometryEvaluationStatus::Evaluated);
    let evaluation = result.evaluation.ok_or("evaluation missing")?;
    assert_eq!(evaluation.summary.edge_count, 18);
    assert_eq!(evaluation.summary.unsampled_edge_count, 0);
    let bounds = evaluation
        .bounding_box
        .ok_or("bounding box missing")?
        .bounds;
    let expected_min = [-64.815_230_330_804_52, -40.827_856_510_186_03, 0.0];
    let expected_max = [63.500_890_129_780_08, 39.106_775_907_883_08, 20.0];
    for axis in 0..3 {
        assert!((bounds.min[axis] - expected_min[axis]).abs() < 1e-6);
        assert!((bounds.max[axis] - expected_max[axis]).abs() < 1e-6);
    }
    for edge in &evaluation.edges {
        if edge.is_derived_seam {
            assert_eq!(edge.smoothness, EdgeSmoothness::Tangent);
        } else {
            assert_eq!(edge.smoothness, EdgeSmoothness::Sharp, "{}", edge.edge_id);
            let angle = edge.dihedral_angle_deg.ok_or("dihedral missing")?;
            assert!((angle - 90.0).abs() < 1e-6, "{} {angle}", edge.edge_id);
        }
    }
    Ok(())
}
