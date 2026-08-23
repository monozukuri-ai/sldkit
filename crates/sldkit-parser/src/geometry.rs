use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};

use cadmpeg_codec_sldprt::SldprtCodec;
use cadmpeg_core::{
    CodecError,
    decode::{DecodeMode, DecodePolicy, ResourceLimits as DecoderResourceLimits},
};
use cadmpeg_ir::{
    Codec, DecodeOptions, Exactness,
    document::CadIr,
    ids::{BodyId, CoedgeId, EdgeId, FaceId, LoopId, RegionId, ShellId, VertexId},
    topology::BodyKind,
    validate_neutral_with_source_fidelity,
};
use flate2::read::ZlibDecoder;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sldkit_container::extract_bytes as extract_container_bytes;
use sldkit_core::{
    Diagnostic, DiagnosticKind, DiagnosticSeverity, DocumentKind, Envelope, ExtractionMode,
    GeometryBody, GeometryByteCoverage, GeometryByteDomain, GeometryByteOffsetBasis,
    GeometryBytePartitionStatus, GeometryByteStorage, GeometryCarrier, GeometryCarrierDomain,
    GeometryCoedge, GeometryConfigurationState, GeometryConstruction, GeometryConstructionDomain,
    GeometryDocument, GeometryEdge, GeometryEntityProvenance, GeometryExactness, GeometryFace,
    GeometryFidelityReport, GeometryFinding, GeometryLoop, GeometryLoss, GeometryModel,
    GeometryPcurveState, GeometryPcurveUse, GeometryPoint, GeometryRawRecord, GeometryRegion,
    GeometryResult, GeometryShell, GeometrySourceObject, GeometryStatus, GeometryStreamCandidate,
    GeometryStreamRole, GeometryStreamSelection, GeometryTessellation, GeometryTessellationChannel,
    GeometryTessellationTextureAssignment, GeometryTessellationTriangleGroup,
    GeometryTopologyMetrics, GeometryVertex, GeometryVertexUse, InventoryEntryState,
    InventoryResult, InventoryStatus, ResourceLimits, SourceInfo, SourceInputKind,
};

const DECODER_NAME: &str = "cadmpeg-codec-sldprt";
const DECODER_VERSION: &str = "0.5.3";
const MAX_NESTED_STREAM_PROBES: u64 = 1_024;
const PARASOLID_MAGIC: &[u8; 4] = b"PS\0\0";
const WRAPPED_PARASOLID_MAGIC_PREFIX: [u8; 16] = [
    0x23, 0x1d, 0xd5, 0x71, 0xda, 0x81, 0x48, 0xa2, 0xa8, 0x58, 0x98, 0xb2, 0x1b, 0x89, 0xef, 0x99,
];

#[allow(clippy::too_many_lines)]
pub(crate) fn decode(
    data: &[u8],
    filename: Option<&str>,
    input_kind: SourceInputKind,
    limits: &ResourceLimits,
) -> GeometryResult {
    let inspection = super::inspect_bytes(data, filename, limits);
    if let Some(result) = inspection_failure(&inspection) {
        return result;
    }
    let Some(inventory) = inspection.inventory.as_ref() else {
        return unsupported(
            inspection.diagnostics,
            "geometry.container_unsupported",
            "geometry decode requires a recognized modern container inventory",
        );
    };
    if inventory.envelope != Envelope::ModernChunk {
        return unsupported(
            inspection.diagnostics,
            "geometry.envelope_unsupported",
            "geometry decode is limited to modern SLDPRT containers",
        );
    }

    let filename_kind = super::document_kind_hint(filename);
    let semantic = super::modern_semantics::decode(data, inventory, &filename_kind, limits);
    if semantic.rejected {
        let mut diagnostics = inspection.diagnostics;
        diagnostics.extend(semantic.diagnostics);
        return GeometryResult {
            status: GeometryStatus::Rejected,
            geometry: None,
            diagnostics,
        };
    }
    if matches!(
        semantic.document_kind.value,
        DocumentKind::Assembly | DocumentKind::Drawing
    ) {
        return unsupported(
            inspection.diagnostics,
            "geometry.document_kind_unsupported",
            "geometry decode accepts Part documents only",
        );
    }

    let options = DecodeOptions {
        container_only: false,
        policy: decoder_policy(limits),
    };
    let mut reader = Cursor::new(data);
    let decoded = match SldprtCodec.decode(&mut reader, &options) {
        Ok(decoded) => decoded,
        Err(error) => return decoder_failure(inspection.diagnostics, &error),
    };

    let validation = validate_neutral_with_source_fidelity(
        decoded.ir(),
        decoded.source_fidelity(),
        decoded.report().losses.clone(),
    );
    let model = match map_model(decoded.ir(), decoded.source_fidelity()) {
        Ok(model) => model,
        Err(error) => {
            let mut diagnostics = inspection.diagnostics;
            diagnostics.push(
                Diagnostic::new(
                    "geometry.model_serialization_failed",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "decoded geometry could not be converted into the sldkit source model",
                )
                .with_detail("error", error.to_string()),
            );
            return GeometryResult {
                status: GeometryStatus::Rejected,
                geometry: None,
                diagnostics,
            };
        }
    };

    let active_stream = decoded
        .ir()
        .source
        .as_ref()
        .and_then(|source| source.attributes.get("active_parasolid_block"))
        .map(String::as_str);
    let source_streams = stream_candidates(
        &inspection,
        active_stream,
        decoded.report().geometry_transferred,
    );
    let raw_records = decoded
        .source_fidelity()
        .retained_records
        .iter()
        .filter(|record| record.id != "sldprt:file:source-image#0")
        .map(|record| GeometryRawRecord {
            id: record.id.clone(),
            stream: record.stream.clone(),
            offset: record.offset,
            byte_len: record.byte_len,
            sha256: record.sha256.clone(),
            data_retained: record.data.is_some(),
        })
        .collect::<Vec<_>>();
    let losses = decoded
        .report()
        .losses
        .iter()
        .map(map_loss)
        .collect::<Vec<_>>();
    let validation_findings = validation
        .findings
        .iter()
        .map(|finding| GeometryFinding {
            check: finding.check.to_string(),
            severity: finding.severity.to_string(),
            message: finding.message.clone(),
            entity_id: finding.entity.clone(),
        })
        .collect::<Vec<_>>();
    let byte_domains = byte_domains(data, &source_streams, limits);
    let byte_coverage = byte_coverage(data, &source_streams, &byte_domains, &raw_records, &model);
    let fidelity = GeometryFidelityReport {
        decoder: DECODER_NAME.to_owned(),
        decoder_version: DECODER_VERSION.to_owned(),
        geometry_transferred: decoded.report().geometry_transferred,
        entity_counts: validation
            .entity_counts
            .iter()
            .map(|(name, count)| (name.clone(), u64::try_from(*count).unwrap_or(u64::MAX)))
            .collect(),
        byte_domains,
        byte_coverage,
        losses,
        validation_findings,
    };
    let status = geometry_status(&fidelity, &model);
    let mut diagnostics = inspection.diagnostics;
    if !fidelity.geometry_transferred {
        diagnostics.push(Diagnostic::new(
            "geometry.not_transferred",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unsupported,
            "no Parasolid body stream produced a typed B-Rep topology graph",
        ));
    } else if status == GeometryStatus::Partial {
        diagnostics.push(Diagnostic::new(
            "geometry.partial",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Preserved,
            "B-Rep geometry was transferred with explicit geometry or topology losses",
        ));
    }
    if fidelity.geometry_transferred && fidelity.byte_domains.is_empty() {
        diagnostics.push(Diagnostic::new(
            "geometry.byte_domain_unavailable",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unsupported,
            "typed geometry was transferred, but its nested Parasolid byte domain could not be established",
        ));
    }
    if byte_coverage.partition_status == GeometryBytePartitionStatus::Incomplete {
        diagnostics.push(Diagnostic::new(
            "geometry.byte_partition_incomplete",
            DiagnosticSeverity::Info,
            DiagnosticKind::Unsupported,
            "entity byte locations are available, but source record lengths do not yet provide a complete typed-versus-uninterpreted range partition",
        ));
    }

    let source = SourceInfo {
        input_kind,
        label: filename.map(str::to_owned),
        byte_len: u64::try_from(data.len()).unwrap_or(u64::MAX),
        sha256: sha256_hex(data),
    };
    GeometryResult {
        status,
        geometry: Some(GeometryDocument {
            source,
            length_unit: "millimeter".to_owned(),
            source_streams,
            model,
            configurations: map_configurations(decoded.ir()),
            topology_metrics: topology_metrics(decoded.ir()),
            raw_records,
            fidelity,
        }),
        diagnostics,
    }
}

fn inspection_failure(inspection: &InventoryResult) -> Option<GeometryResult> {
    let status = match inspection.status {
        InventoryStatus::Rejected => GeometryStatus::Rejected,
        InventoryStatus::Malformed => GeometryStatus::Malformed,
        InventoryStatus::Unsupported if inspection.inventory.is_none() => {
            GeometryStatus::Unsupported
        }
        InventoryStatus::Complete | InventoryStatus::Partial | InventoryStatus::Unsupported => {
            return None;
        }
    };
    Some(GeometryResult {
        status,
        geometry: None,
        diagnostics: inspection.diagnostics.clone(),
    })
}

fn unsupported(
    mut diagnostics: Vec<Diagnostic>,
    code: &'static str,
    message: &'static str,
) -> GeometryResult {
    diagnostics.push(Diagnostic::new(
        code,
        DiagnosticSeverity::Warning,
        DiagnosticKind::Unsupported,
        message,
    ));
    GeometryResult {
        status: GeometryStatus::Unsupported,
        geometry: None,
        diagnostics,
    }
}

fn decoder_failure(mut diagnostics: Vec<Diagnostic>, error: &CodecError) -> GeometryResult {
    let (status, kind, code) = match error {
        CodecError::WrongFormat(_) | CodecError::NotImplemented(_) => (
            GeometryStatus::Unsupported,
            DiagnosticKind::Unsupported,
            "geometry.decoder_unsupported",
        ),
        CodecError::ResourceLimit(_) => (
            GeometryStatus::Rejected,
            DiagnosticKind::Fatal,
            "geometry.decoder_resource_limit",
        ),
        CodecError::Malformed(_) | CodecError::Truncated { .. } => (
            GeometryStatus::Malformed,
            DiagnosticKind::Malformed,
            "geometry.decoder_malformed",
        ),
        CodecError::Io(_) => (
            GeometryStatus::Malformed,
            DiagnosticKind::Malformed,
            "geometry.decoder_io",
        ),
        _ => (
            GeometryStatus::Malformed,
            DiagnosticKind::Malformed,
            "geometry.decoder_failed",
        ),
    };
    diagnostics.push(
        Diagnostic::new(
            code,
            DiagnosticSeverity::Error,
            kind,
            "the bounded geometry decoder could not process the input",
        )
        .with_detail("error", error.to_string()),
    );
    GeometryResult {
        status,
        geometry: None,
        diagnostics,
    }
}

fn decoder_policy(limits: &ResourceLimits) -> DecodePolicy {
    let max_entities = limits.max_stream_count.saturating_mul(64);
    let max_collection_items = limits.max_stream_count.saturating_mul(16);
    DecodePolicy {
        mode: DecodeMode::Salvage,
        limits: DecoderResourceLimits {
            max_input_bytes: limits.max_file_size,
            max_decompressed_bytes_total: limits.max_total_uncompressed_bytes,
            max_decompressed_bytes_per_expand: limits
                .max_file_size
                .min(limits.max_total_uncompressed_bytes),
            max_materialized_bytes: limits.max_total_uncompressed_bytes / 2,
            max_retained_bytes: limits.max_file_size,
            max_entities,
            max_collection_items,
            max_recursion_depth: u64::from(limits.max_nesting_depth),
            max_work_units: limits.max_total_uncompressed_bytes,
        },
    }
}

fn geometry_status(fidelity: &GeometryFidelityReport, model: &GeometryModel) -> GeometryStatus {
    if !fidelity.geometry_transferred || model.bodies.is_empty() {
        return GeometryStatus::Partial;
    }
    let lossy_geometry = fidelity.losses.iter().any(|loss| {
        matches!(loss.category.as_str(), "geometry" | "topology") && loss.severity != "info"
    });
    let invalid = fidelity
        .validation_findings
        .iter()
        .any(|finding| matches!(finding.severity.as_str(), "error" | "blocking"));
    if lossy_geometry || invalid {
        GeometryStatus::Partial
    } else {
        GeometryStatus::Decoded
    }
}

#[allow(clippy::too_many_lines)]
fn map_model(
    ir: &CadIr,
    fidelity: &cadmpeg_ir::SourceFidelity,
) -> Result<GeometryModel, serde_json::Error> {
    let annotations = &fidelity.annotations;
    let bodies = ir
        .model
        .bodies
        .iter()
        .map(|body| {
            Ok(GeometryBody {
                id: body.id.0.clone(),
                kind: lower_debug(body.kind),
                region_ids: ids(&body.regions),
                transform: body
                    .transform
                    .as_ref()
                    .map(serde_json::to_value)
                    .transpose()?,
                name: body.name.clone(),
                color: body.color.as_ref().map(map_color),
                visible: body.visible,
                provenance: provenance(&body.id.0, annotations),
            })
        })
        .collect::<Result<Vec<_>, serde_json::Error>>()?;
    let regions = ir
        .model
        .regions
        .iter()
        .map(|region| GeometryRegion {
            id: region.id.0.clone(),
            body_id: region.body.0.clone(),
            shell_ids: ids(&region.shells),
            provenance: provenance(&region.id.0, annotations),
        })
        .collect();
    let shells = ir
        .model
        .shells
        .iter()
        .map(|shell| GeometryShell {
            id: shell.id.0.clone(),
            region_id: shell.region.0.clone(),
            face_ids: ids(&shell.faces),
            wire_edge_ids: ids(&shell.wire_edges),
            free_vertex_ids: ids(&shell.free_vertices),
            provenance: provenance(&shell.id.0, annotations),
        })
        .collect();
    let faces = ir
        .model
        .faces
        .iter()
        .map(|face| GeometryFace {
            id: face.id.0.clone(),
            shell_id: face.shell.0.clone(),
            surface_id: face.surface.0.clone(),
            sense: lower_debug(face.sense),
            loop_ids: ids(&face.loops),
            name: face.name.clone(),
            color: face.color.as_ref().map(map_color),
            tolerance: face.tolerance,
            provenance: provenance(&face.id.0, annotations),
        })
        .collect();
    let loops = ir
        .model
        .loops
        .iter()
        .map(|item| GeometryLoop {
            id: item.id.0.clone(),
            face_id: item.face.0.clone(),
            boundary_role: lower_debug(item.boundary_role),
            coedge_ids: ids(&item.coedges),
            vertex_uses: item
                .vertex_uses
                .iter()
                .map(|vertex| GeometryVertexUse {
                    vertex_id: vertex.vertex.0.clone(),
                    after_coedge_id: vertex.after.as_ref().map(|id| id.0.clone()),
                    pcurves: map_pcurve_uses(&vertex.pcurves),
                })
                .collect(),
            provenance: provenance(&item.id.0, annotations),
        })
        .collect();
    let coedges = ir
        .model
        .coedges
        .iter()
        .map(|coedge| GeometryCoedge {
            id: coedge.id.0.clone(),
            loop_id: coedge.owner_loop.0.clone(),
            edge_id: coedge.edge.0.clone(),
            next_id: coedge.next.0.clone(),
            previous_id: coedge.previous.0.clone(),
            radial_next_id: coedge.radial_next.0.clone(),
            sense: lower_debug(coedge.sense),
            pcurves: map_pcurve_uses(&coedge.pcurves),
            use_curve_id: coedge.use_curve.as_ref().map(|id| id.0.clone()),
            use_curve_parameter_range: coedge.use_curve_parameter_range,
            provenance: provenance(&coedge.id.0, annotations),
        })
        .collect();
    let edges = ir
        .model
        .edges
        .iter()
        .map(|edge| GeometryEdge {
            id: edge.id.0.clone(),
            curve_id: edge.curve.as_ref().map(|id| id.0.clone()),
            start_vertex_id: edge.start.0.clone(),
            end_vertex_id: edge.end.0.clone(),
            parameter_range: edge.param_range,
            tolerance: edge.tolerance,
            provenance: provenance(&edge.id.0, annotations),
        })
        .collect();
    let vertices = ir
        .model
        .vertices
        .iter()
        .map(|vertex| GeometryVertex {
            id: vertex.id.0.clone(),
            point_id: vertex.point.0.clone(),
            tolerance: vertex.tolerance,
            provenance: provenance(&vertex.id.0, annotations),
        })
        .collect();
    let points = ir
        .model
        .points
        .iter()
        .map(|point| GeometryPoint {
            id: point.id.0.clone(),
            position: [point.position.x, point.position.y, point.position.z],
            source_object: point.source_object.as_ref().map(map_source_object),
            provenance: provenance(&point.id.0, annotations),
        })
        .collect();

    let mut carriers = Vec::with_capacity(
        ir.model.surfaces.len() + ir.model.curves.len() + ir.model.pcurves.len(),
    );
    for surface in &ir.model.surfaces {
        carriers.push(map_carrier(
            &surface.id.0,
            GeometryCarrierDomain::Surface,
            &surface.geometry,
            surface.source_object.as_ref(),
            None,
            annotations,
        )?);
    }
    for curve in &ir.model.curves {
        carriers.push(map_carrier(
            &curve.id.0,
            GeometryCarrierDomain::Curve,
            &curve.geometry,
            curve.source_object.as_ref(),
            None,
            annotations,
        )?);
    }
    for pcurve in &ir.model.pcurves {
        carriers.push(map_carrier(
            &pcurve.id.0,
            GeometryCarrierDomain::Pcurve,
            &pcurve.geometry,
            None,
            Some(GeometryPcurveState {
                wrapper_reversed: pcurve.wrapper_reversed,
                native_tail_flags: pcurve.native_tail_flags,
                parameter_range: pcurve.parameter_range,
                fit_tolerance: pcurve.fit_tolerance,
            }),
            annotations,
        )?);
    }
    carriers.sort_by(|left, right| left.id.cmp(&right.id));

    let mut constructions =
        Vec::with_capacity(ir.model.procedural_surfaces.len() + ir.model.procedural_curves.len());
    for construction in &ir.model.procedural_surfaces {
        constructions.push(map_construction(
            &construction.id.0,
            GeometryConstructionDomain::Surface,
            &construction.surface.0,
            &construction.definition,
            construction.cache_fit_tolerance,
            construction.record_bounds,
            annotations,
        )?);
    }
    for construction in &ir.model.procedural_curves {
        constructions.push(map_construction(
            &construction.id.0,
            GeometryConstructionDomain::Curve,
            &construction.curve.0,
            &construction.definition,
            construction.cache_fit_tolerance,
            None,
            annotations,
        )?);
    }
    constructions.sort_by(|left, right| left.id.cmp(&right.id));

    let tessellations = ir
        .model
        .tessellations
        .iter()
        .map(|mesh| GeometryTessellation {
            id: mesh.id.clone(),
            body_id: mesh.body.as_ref().map(|id| id.0.clone()),
            face_ids: ids(&mesh.faces),
            chordal_deflection: mesh.chordal_deflection,
            source_object: mesh.source_object.as_ref().map(map_source_object),
            vertices: mesh
                .vertices
                .iter()
                .map(|point| [point.x, point.y, point.z])
                .collect(),
            triangles: mesh.triangles.clone(),
            feature_edges: mesh.feature_edges.clone(),
            strip_lengths: mesh.strip_lengths.clone(),
            normals: mesh
                .normals
                .iter()
                .map(|normal| [normal.x, normal.y, normal.z])
                .collect(),
            corner_normals: mesh
                .corner_normals
                .iter()
                .map(|normal| [normal.x, normal.y, normal.z])
                .collect(),
            triangle_groups: mesh
                .triangle_groups
                .iter()
                .map(|group| GeometryTessellationTriangleGroup {
                    source_id: group.source_id.clone(),
                    triangles: group.triangles.clone(),
                })
                .collect(),
            texture_assignments: mesh
                .texture_assignments
                .iter()
                .map(|assignment| GeometryTessellationTextureAssignment {
                    source_id: assignment.source_id.clone(),
                    texture_id: assignment.texture.0.clone(),
                    triangles: assignment.triangles.clone(),
                })
                .collect(),
            channels: mesh
                .channels
                .iter()
                .map(|channel| GeometryTessellationChannel {
                    domain: lower_debug(channel.domain),
                    item_size: channel.item_size,
                    kind: channel.kind,
                    flags: channel.flags,
                    count: channel.count,
                    byte_len: u64::try_from(channel.data.len()).unwrap_or(u64::MAX),
                    sha256: sha256_hex(&channel.data),
                    indices: channel.indices.clone(),
                })
                .collect(),
            provenance: provenance(&mesh.id, annotations),
        })
        .collect();

    Ok(GeometryModel {
        bodies,
        regions,
        shells,
        faces,
        loops,
        coedges,
        edges,
        vertices,
        points,
        carriers,
        constructions,
        tessellations,
    })
}

fn map_carrier<T: Serialize>(
    id: &str,
    domain: GeometryCarrierDomain,
    geometry: &T,
    source_object: Option<&cadmpeg_ir::SourceObjectAssociation>,
    pcurve_state: Option<GeometryPcurveState>,
    annotations: &cadmpeg_ir::Annotations,
) -> Result<GeometryCarrier, serde_json::Error> {
    let definition = serde_json::to_value(geometry)?;
    let kind = definition
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();
    let raw_record_id = raw_record_id(&definition);
    Ok(GeometryCarrier {
        id: id.to_owned(),
        domain,
        kind,
        definition,
        raw_record_id,
        source_object: source_object.map(map_source_object),
        pcurve_state,
        provenance: provenance(id, annotations),
    })
}

fn map_construction<T: Serialize>(
    id: &str,
    domain: GeometryConstructionDomain,
    produced_carrier_id: &str,
    source_definition: &T,
    cache_fit_tolerance: Option<f64>,
    record_bounds: Option<[Option<f64>; 4]>,
    annotations: &cadmpeg_ir::Annotations,
) -> Result<GeometryConstruction, serde_json::Error> {
    let definition = serde_json::to_value(source_definition)?;
    Ok(GeometryConstruction {
        id: id.to_owned(),
        domain,
        produced_carrier_id: produced_carrier_id.to_owned(),
        raw_record_id: raw_record_id(&definition),
        definition,
        cache_fit_tolerance,
        record_bounds,
        provenance: provenance(id, annotations),
    })
}

fn raw_record_id(definition: &Value) -> Option<String> {
    definition
        .get("record")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

const fn map_color(color: &cadmpeg_ir::topology::Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

fn map_source_object(source: &cadmpeg_ir::SourceObjectAssociation) -> GeometrySourceObject {
    GeometrySourceObject {
        format: source.format.clone(),
        object_id: source.object_id.clone(),
        name: source.name.clone(),
        color: source.color.as_ref().map(map_color),
        visible: source.visible,
        layer: source.layer.clone(),
        instance_path: source.instance_path.clone(),
    }
}

fn map_pcurve_uses(values: &[cadmpeg_ir::topology::PcurveUse]) -> Vec<GeometryPcurveUse> {
    values
        .iter()
        .map(|value| GeometryPcurveUse {
            pcurve_id: value.pcurve.0.clone(),
            isoparametric: value.isoparametric,
            parameter_range: value.parameter_range,
        })
        .collect()
}

fn provenance(id: &str, annotations: &cadmpeg_ir::Annotations) -> GeometryEntityProvenance {
    let location = annotations.provenance.get(id);
    let exactness = annotations.exactness.get(id);
    GeometryEntityProvenance {
        stream: location.and_then(|location| {
            usize::try_from(location.stream)
                .ok()
                .and_then(|index| annotations.streams.get(index))
                .cloned()
        }),
        offset: location.map(|location| location.offset),
        tag: location.and_then(|location| location.tag.clone()),
        exactness: exactness.map_or(GeometryExactness::Unknown, |note| {
            map_exactness(note.entity)
        }),
        field_exactness: exactness
            .map(|note| {
                note.fields
                    .iter()
                    .map(|(field, value)| (field.clone(), map_exactness(*value)))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

const fn map_exactness(value: Exactness) -> GeometryExactness {
    match value {
        Exactness::ByteExact => GeometryExactness::ByteExact,
        Exactness::Derived => GeometryExactness::Derived,
        Exactness::Inferred => GeometryExactness::Inferred,
        Exactness::Unknown => GeometryExactness::Unknown,
    }
}

fn map_configurations(ir: &CadIr) -> Vec<GeometryConfigurationState> {
    ir.model
        .configurations
        .iter()
        .map(|configuration| GeometryConfigurationState {
            id: configuration.id.0.clone(),
            ordinal: configuration.ordinal,
            active: configuration.active.resolved(),
            source_index: configuration.source_index,
            name: configuration.name.resolved().map(str::to_owned),
            body_ids: configuration.bodies.resolved().map(ids),
        })
        .collect()
}

fn stream_candidates(
    inspection: &InventoryResult,
    active_stream: Option<&str>,
    geometry_transferred: bool,
) -> Vec<GeometryStreamCandidate> {
    let Some(inventory) = inspection.inventory.as_ref() else {
        return Vec::new();
    };
    let mut candidates = inventory
        .entries
        .iter()
        .filter(|entry| entry.state == InventoryEntryState::Decoded)
        .filter_map(|entry| {
            let path = entry.path.as_deref()?;
            let lower = path.to_ascii_lowercase();
            let role = if lower.contains("deltas")
                && !lower.contains("ghost")
                && !lower.contains("resolvedfeatures")
            {
                GeometryStreamRole::ParasolidDeltas
            } else if lower.contains("partition")
                && !lower.contains("ghost")
                && !lower.contains("resolvedfeatures")
            {
                GeometryStreamRole::ParasolidPartition
            } else if lower.contains("displaylists") || lower.contains("lwdata") {
                GeometryStreamRole::Tessellation
            } else {
                return None;
            };
            let is_active = active_stream.is_some_and(|active| active.eq_ignore_ascii_case(path));
            let (selection, evidence) = if is_active {
                (
                    GeometryStreamSelection::Active,
                    vec!["decoder.source.active_parasolid_block".to_owned()],
                )
            } else if role == GeometryStreamRole::Tessellation {
                (
                    GeometryStreamSelection::Supporting,
                    vec!["stream_path.display_tessellation_candidate".to_owned()],
                )
            } else if geometry_transferred {
                (
                    GeometryStreamSelection::Alternate,
                    vec!["decoder.body_site.alternate".to_owned()],
                )
            } else {
                (
                    GeometryStreamSelection::Candidate,
                    vec!["stream_path.parasolid_candidate".to_owned()],
                )
            };
            Some(GeometryStreamCandidate {
                entry_id: entry.id.clone(),
                stream_path: path.to_owned(),
                role,
                selection,
                decoded_size: entry.decoded_size,
                decoded_sha256: entry.decoded_sha256.clone(),
                selection_evidence: evidence,
            })
        })
        .collect::<Vec<_>>();
    if active_stream.is_none() && geometry_transferred {
        let partition_indices = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.role == GeometryStreamRole::ParasolidPartition)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if let [index] = partition_indices.as_slice() {
            candidates[*index].selection = GeometryStreamSelection::Active;
            candidates[*index].selection_evidence =
                vec!["unique_decoded_partition_candidate".to_owned()];
        }
    }
    candidates
}

#[derive(Debug)]
struct NestedParasolidStream {
    outer_payload_offset: usize,
    storage: GeometryByteStorage,
    bytes: Vec<u8>,
    description: String,
    schema: String,
    body_offset: usize,
    role: GeometryStreamRole,
}

fn byte_domains(
    data: &[u8],
    streams: &[GeometryStreamCandidate],
    limits: &ResourceLimits,
) -> Vec<GeometryByteDomain> {
    let mut domains = Vec::new();
    for stream in streams.iter().filter(|stream| {
        stream.selection == GeometryStreamSelection::Active
            && matches!(
                stream.role,
                GeometryStreamRole::ParasolidPartition | GeometryStreamRole::ParasolidDeltas
            )
    }) {
        let extraction =
            extract_container_bytes(data, &stream.entry_id, ExtractionMode::Decoded, limits);
        let Some(payload) = extraction.data else {
            continue;
        };
        for nested in nested_parasolid_streams(&payload, limits) {
            let body = &nested.bytes[nested.body_offset..];
            let ordinal = domains.len();
            domains.push(GeometryByteDomain {
                id: format!("{}:parasolid-body:{ordinal:04}", stream.entry_id),
                container_entry_id: stream.entry_id.clone(),
                stream_path: stream.stream_path.clone(),
                role: nested.role,
                storage: nested.storage,
                outer_payload_offset: saturating_u64(nested.outer_payload_offset),
                description: nested.description,
                schema: nested.schema,
                stream_byte_len: saturating_u64(nested.bytes.len()),
                stream_sha256: sha256_hex(&nested.bytes),
                body_offset: saturating_u64(nested.body_offset),
                byte_len: saturating_u64(body.len()),
                sha256: sha256_hex(body),
                offset_basis: GeometryByteOffsetBasis::ParasolidBody,
            });
        }
    }
    domains
}

fn nested_parasolid_streams(payload: &[u8], limits: &ResourceLimits) -> Vec<NestedParasolidStream> {
    let direct_starts = payload
        .windows(PARASOLID_MAGIC.len())
        .enumerate()
        .filter_map(|(offset, value)| (value == PARASOLID_MAGIC).then_some(offset))
        .filter(|offset| parasolid_header(&payload[*offset..]).is_some())
        .collect::<Vec<_>>();
    if !direct_starts.is_empty() {
        return direct_starts
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(index, start)| {
                let end = direct_starts
                    .get(index + 1)
                    .copied()
                    .unwrap_or(payload.len());
                nested_parasolid_stream(
                    start,
                    GeometryByteStorage::Direct,
                    payload.get(start..end)?.to_vec(),
                )
            })
            .collect();
    }

    if !payload
        .windows(WRAPPED_PARASOLID_MAGIC_PREFIX.len())
        .any(|value| value == WRAPPED_PARASOLID_MAGIC_PREFIX)
    {
        return Vec::new();
    }

    let mut nested = Vec::new();
    let mut attempts = 0_u64;
    let attempt_limit = limits.max_stream_count.min(MAX_NESTED_STREAM_PROBES);
    let mut remaining_expand_bytes = limits.max_total_uncompressed_bytes;
    for offset in 0..payload.len().saturating_sub(1) {
        if payload[offset] != 0x78 || !matches!(payload[offset + 1], 0x01 | 0x9c | 0xda) {
            continue;
        }
        if attempts >= attempt_limit || remaining_expand_bytes == 0 {
            break;
        }
        attempts = attempts.saturating_add(1);
        let (inflated, expanded_bytes) =
            inflate_zlib_bounded(&payload[offset..], limits, remaining_expand_bytes);
        remaining_expand_bytes = remaining_expand_bytes.saturating_sub(expanded_bytes);
        let Some(bytes) = inflated else {
            continue;
        };
        if !bytes.starts_with(PARASOLID_MAGIC)
            || nested
                .iter()
                .any(|candidate: &NestedParasolidStream| candidate.bytes == bytes)
        {
            continue;
        }
        if let Some(candidate) =
            nested_parasolid_stream(offset, GeometryByteStorage::WrappedZlib, bytes)
        {
            nested.push(candidate);
        }
    }
    nested
}

fn nested_parasolid_stream(
    outer_payload_offset: usize,
    storage: GeometryByteStorage,
    bytes: Vec<u8>,
) -> Option<NestedParasolidStream> {
    let (description, schema, body_offset) = parasolid_header(&bytes)?;
    let lower = description.to_ascii_lowercase();
    let role = if lower.contains("partition") {
        GeometryStreamRole::ParasolidPartition
    } else if lower.contains("deltas") {
        GeometryStreamRole::ParasolidDeltas
    } else {
        return None;
    };
    Some(NestedParasolidStream {
        outer_payload_offset,
        storage,
        bytes,
        description,
        schema,
        body_offset,
        role,
    })
}

fn parasolid_header(payload: &[u8]) -> Option<(String, String, usize)> {
    if !payload.starts_with(PARASOLID_MAGIC) {
        return None;
    }
    let description_len = usize::from(u16::from_be_bytes([*payload.get(4)?, *payload.get(5)?]));
    let description_start = 6_usize;
    let description_end = description_start.checked_add(description_len)?;
    let description =
        String::from_utf8_lossy(payload.get(description_start..description_end)?).into_owned();
    let search_end = description_end.saturating_add(64).min(payload.len());
    let schema_relative = payload
        .get(description_end..search_end)?
        .windows(4)
        .position(|value| value == b"SCH_")?;
    let schema_start = description_end.checked_add(schema_relative)?;
    let schema_len = usize::from(*payload.get(schema_start.checked_sub(1)?)?);
    let schema_end = schema_start.checked_add(schema_len)?;
    let schema = String::from_utf8_lossy(payload.get(schema_start..schema_end)?).into_owned();
    Some((description, schema, schema_end))
}

fn inflate_zlib_bounded(
    payload: &[u8],
    limits: &ResourceLimits,
    remaining_expand_bytes: u64,
) -> (Option<Vec<u8>>, u64) {
    let input_bytes = saturating_u64(payload.len());
    let limit = limits
        .max_file_size
        .min(limits.max_total_uncompressed_bytes)
        .min(remaining_expand_bytes)
        .min(input_bytes.saturating_mul(limits.max_compression_ratio));
    if limit == 0 {
        return (None, 0);
    }
    let mut decoder = ZlibDecoder::new(payload).take(limit.saturating_add(1));
    let mut output = Vec::new();
    let read_succeeded = decoder.read_to_end(&mut output).is_ok();
    let expanded_bytes = saturating_u64(output.len());
    (
        (read_succeeded && expanded_bytes <= limit).then_some(output),
        expanded_bytes,
    )
}

fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn byte_coverage(
    data: &[u8],
    streams: &[GeometryStreamCandidate],
    byte_domains: &[GeometryByteDomain],
    raw_records: &[GeometryRawRecord],
    model: &GeometryModel,
) -> GeometryByteCoverage {
    let active_stream_bytes = streams
        .iter()
        .filter(|stream| stream.selection == GeometryStreamSelection::Active)
        .filter_map(|stream| stream.decoded_size)
        .fold(0_u64, u64::saturating_add);
    let (located_entity_count, unique_location_count) = located_entity_counts(model, streams);
    GeometryByteCoverage {
        source_bytes: u64::try_from(data.len()).unwrap_or(u64::MAX),
        candidate_stream_bytes: streams
            .iter()
            .filter_map(|stream| stream.decoded_size)
            .fold(0_u64, u64::saturating_add),
        active_stream_bytes,
        partition_domain_bytes: byte_domains
            .iter()
            .map(|domain| domain.byte_len)
            .fold(0_u64, u64::saturating_add),
        retained_record_bytes: raw_records
            .iter()
            .map(|record| record.byte_len)
            .fold(0_u64, u64::saturating_add),
        located_entity_count,
        unique_location_count,
        classified_active_bytes: 0,
        unclassified_active_bytes: byte_domains
            .iter()
            .map(|domain| domain.byte_len)
            .fold(0_u64, u64::saturating_add),
        partition_status: GeometryBytePartitionStatus::Incomplete,
        typed_bytes: None,
        uninterpreted_bytes: None,
    }
}

fn located_entity_counts(model: &GeometryModel, streams: &[GeometryStreamCandidate]) -> (u64, u64) {
    let active_streams = streams
        .iter()
        .filter(|stream| stream.selection == GeometryStreamSelection::Active)
        .map(|stream| stream.stream_path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut located = 0_u64;
    let mut unique = BTreeSet::new();
    let mut observe = |provenance: &GeometryEntityProvenance| {
        if matches!(
            provenance.exactness,
            GeometryExactness::Derived | GeometryExactness::Inferred
        ) {
            return;
        }
        let (Some(stream), Some(offset)) = (&provenance.stream, provenance.offset) else {
            return;
        };
        let normalized = stream.to_ascii_lowercase();
        if active_streams.contains(&normalized) {
            located = located.saturating_add(1);
            unique.insert((normalized, offset));
        }
    };
    for item in &model.bodies {
        observe(&item.provenance);
    }
    for item in &model.regions {
        observe(&item.provenance);
    }
    for item in &model.shells {
        observe(&item.provenance);
    }
    for item in &model.faces {
        observe(&item.provenance);
    }
    for item in &model.loops {
        observe(&item.provenance);
    }
    for item in &model.coedges {
        observe(&item.provenance);
    }
    for item in &model.edges {
        observe(&item.provenance);
    }
    for item in &model.vertices {
        observe(&item.provenance);
    }
    for item in &model.points {
        observe(&item.provenance);
    }
    for item in &model.carriers {
        observe(&item.provenance);
    }
    for item in &model.constructions {
        observe(&item.provenance);
    }
    for item in &model.tessellations {
        observe(&item.provenance);
    }
    (located, saturating_len(unique.len()))
}

fn map_loss(loss: &cadmpeg_ir::LossNote) -> GeometryLoss {
    GeometryLoss {
        code: format!("{}/{}", loss.code.namespace, loss.code.code),
        taxonomy: loss.code.taxonomy.as_str().to_owned(),
        category: loss.code.category().to_string(),
        severity: loss.severity.to_string(),
        message: loss.message.clone(),
        stream: loss.provenance.as_ref().map(|value| value.stream.clone()),
        offset: loss.provenance.as_ref().map(|value| value.offset),
        tag: loss.provenance.as_ref().and_then(|value| value.tag.clone()),
    }
}

#[allow(clippy::too_many_lines)]
fn topology_metrics(ir: &CadIr) -> Vec<GeometryTopologyMetrics> {
    let regions = ir
        .model
        .regions
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let shells = ir
        .model
        .shells
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let faces = ir
        .model
        .faces
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let loops = ir
        .model
        .loops
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let coedges = ir
        .model
        .coedges
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let edges = ir
        .model
        .edges
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();

    ir.model
        .bodies
        .iter()
        .map(|body| {
            let mut region_ids = BTreeSet::new();
            let mut shell_ids = BTreeSet::new();
            let mut face_ids = BTreeSet::new();
            let mut loop_ids = BTreeSet::new();
            let mut coedge_ids = BTreeSet::new();
            let mut edge_ids = BTreeSet::new();
            let mut vertex_ids = BTreeSet::new();
            for region_id in &body.regions {
                region_ids.insert(region_id.as_str());
                let Some(region) = regions.get(region_id.as_str()) else {
                    continue;
                };
                for shell_id in &region.shells {
                    shell_ids.insert(shell_id.as_str());
                    let Some(shell) = shells.get(shell_id.as_str()) else {
                        continue;
                    };
                    for edge_id in &shell.wire_edges {
                        collect_edge(edge_id, &edges, &mut edge_ids, &mut vertex_ids);
                    }
                    for vertex_id in &shell.free_vertices {
                        vertex_ids.insert(vertex_id.as_str());
                    }
                    for face_id in &shell.faces {
                        face_ids.insert(face_id.as_str());
                        let Some(face) = faces.get(face_id.as_str()) else {
                            continue;
                        };
                        for loop_id in &face.loops {
                            loop_ids.insert(loop_id.as_str());
                            let Some(item) = loops.get(loop_id.as_str()) else {
                                continue;
                            };
                            for vertex_use in &item.vertex_uses {
                                vertex_ids.insert(vertex_use.vertex.as_str());
                            }
                            for coedge_id in &item.coedges {
                                coedge_ids.insert(coedge_id.as_str());
                                if let Some(coedge) = coedges.get(coedge_id.as_str()) {
                                    collect_edge(
                                        &coedge.edge,
                                        &edges,
                                        &mut edge_ids,
                                        &mut vertex_ids,
                                    );
                                }
                            }
                        }
                    }
                }
            }
            let euler_characteristic = matches!(body.kind, BodyKind::Solid | BodyKind::Sheet)
                .then(|| euler(vertex_ids.len(), edge_ids.len(), face_ids.len()))
                .flatten();
            GeometryTopologyMetrics {
                body_id: body.id.0.clone(),
                regions: saturating_len(region_ids.len()),
                shells: saturating_len(shell_ids.len()),
                faces: saturating_len(face_ids.len()),
                loops: saturating_len(loop_ids.len()),
                coedges: saturating_len(coedge_ids.len()),
                edges: saturating_len(edge_ids.len()),
                vertices: saturating_len(vertex_ids.len()),
                euler_characteristic,
            }
        })
        .collect()
}

fn collect_edge<'a>(
    edge_id: &'a EdgeId,
    edges: &BTreeMap<&'a str, &'a cadmpeg_ir::topology::Edge>,
    edge_ids: &mut BTreeSet<&'a str>,
    vertex_ids: &mut BTreeSet<&'a str>,
) {
    edge_ids.insert(edge_id.as_str());
    if let Some(edge) = edges.get(edge_id.as_str()) {
        vertex_ids.insert(edge.start.as_str());
        vertex_ids.insert(edge.end.as_str());
    }
}

fn euler(vertices: usize, edges: usize, faces: usize) -> Option<i64> {
    i64::try_from(vertices)
        .ok()?
        .checked_sub(i64::try_from(edges).ok()?)?
        .checked_add(i64::try_from(faces).ok()?)
}

fn saturating_len(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn ids<T>(values: &[T]) -> Vec<String>
where
    T: AsRefId,
{
    values
        .iter()
        .map(|value| value.id_ref().to_owned())
        .collect()
}

trait AsRefId {
    fn id_ref(&self) -> &str;
}

macro_rules! impl_id_ref {
    ($($type:ty),+ $(,)?) => {
        $(impl AsRefId for $type {
            fn id_ref(&self) -> &str {
                self.as_str()
            }
        })+
    };
}

impl_id_ref!(
    BodyId, RegionId, ShellId, FaceId, LoopId, CoedgeId, EdgeId, VertexId
);

fn lower_debug<T: std::fmt::Debug>(value: T) -> String {
    format!("{value:?}").to_ascii_lowercase()
}

fn sha256_hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(data);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, io::Write};

    use cadmpeg_ir::{
        ConfigurationBodies, Encoder,
        codec::EncodeInput,
        features::{ConfigurationId, DesignConfiguration},
        geometry::{
            Curve, CurveGeometry, NurbsCurve, NurbsSurface, ProceduralSurface,
            ProceduralSurfaceDefinition, Surface, SurfaceGeometry,
        },
        ids::{
            BodyId, CoedgeId, CurveId, EdgeId, FaceId, LoopId, PointId, ProceduralSurfaceId,
            RegionId, ShellId, SurfaceId, VertexId,
        },
        math::Point3,
        topology::{
            Body, BodyKind, Coedge, Edge, Face, Loop, LoopBoundaryRole, Point, Region, Sense,
            Shell, Vertex,
        },
    };
    use flate2::{Compression, write::ZlibEncoder};
    use sldkit_core::{
        ExtractionMode, ExtractionStatus, GeometryByteOffsetBasis, GeometryBytePartitionStatus,
        GeometryByteStorage, GeometryCarrierDomain, GeometryStatus, GeometryStreamRole,
        GeometryStreamSelection, ResourceLimits, SourceInputKind,
    };

    use super::{
        SldprtCodec, WRAPPED_PARASOLID_MAGIC_PREFIX, decode, nested_parasolid_streams, sha256_hex,
    };

    fn framed_parasolid(description: &str, body: &[u8]) -> Vec<u8> {
        let schema = b"SCH_TEST_1_13006";
        let mut bytes = b"PS\0\0".to_vec();
        bytes.extend_from_slice(
            &u16::try_from(description.len())
                .unwrap_or(u16::MAX)
                .to_be_bytes(),
        );
        bytes.extend_from_slice(description.as_bytes());
        bytes.push(u8::try_from(schema.len()).unwrap_or(u8::MAX));
        bytes.extend_from_slice(schema);
        bytes.extend_from_slice(body);
        bytes
    }

    #[test]
    fn nested_parasolid_domains_distinguish_direct_and_bounded_wrapped_storage()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = framed_parasolid("partition", &[0x5a; 4_096]);
        let direct = nested_parasolid_streams(&stream, &ResourceLimits::service());
        assert_eq!(direct.len(), 1);
        assert_eq!(direct[0].storage, GeometryByteStorage::Direct);
        assert_eq!(direct[0].role, GeometryStreamRole::ParasolidPartition);
        assert_eq!(sha256_hex(&direct[0].bytes), sha256_hex(&stream));

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&stream)?;
        let compressed = encoder.finish()?;
        let mut wrapped = WRAPPED_PARASOLID_MAGIC_PREFIX.to_vec();
        wrapped.extend_from_slice(&compressed);
        let decoded = nested_parasolid_streams(&wrapped, &ResourceLimits::service());
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].storage, GeometryByteStorage::WrappedZlib);
        assert_eq!(decoded[0].bytes, stream);

        let mut constrained = ResourceLimits::service();
        constrained.max_compression_ratio = 1;
        assert!(nested_parasolid_streams(&wrapped, &constrained).is_empty());
        constrained = ResourceLimits::service();
        constrained.max_stream_count = 0;
        assert!(nested_parasolid_streams(&wrapped, &constrained).is_empty());
        Ok(())
    }

    fn encoded_cube() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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
        Ok(bytes)
    }

    #[allow(clippy::too_many_lines)]
    fn append_open_nurbs_sheet(ir: &mut cadmpeg_ir::CadIr) -> BodyId {
        let body_id = BodyId("synthetic:sheet:body#0".into());
        let region_id = RegionId("synthetic:sheet:region#0".into());
        let shell_id = ShellId("synthetic:sheet:shell#0".into());
        let face_id = FaceId("synthetic:sheet:face#0".into());
        let loop_id = LoopId("synthetic:sheet:loop#0".into());
        let surface_id = SurfaceId("synthetic:sheet:surface#nurbs".into());
        let point_ids = [
            PointId("synthetic:sheet:point#0".into()),
            PointId("synthetic:sheet:point#1".into()),
            PointId("synthetic:sheet:point#2".into()),
        ];
        let vertex_ids = [
            VertexId("synthetic:sheet:vertex#0".into()),
            VertexId("synthetic:sheet:vertex#1".into()),
            VertexId("synthetic:sheet:vertex#2".into()),
        ];
        let edge_ids = [
            EdgeId("synthetic:sheet:edge#0".into()),
            EdgeId("synthetic:sheet:edge#1".into()),
            EdgeId("synthetic:sheet:edge#2".into()),
        ];
        let coedge_ids = [
            CoedgeId("synthetic:sheet:coedge#0".into()),
            CoedgeId("synthetic:sheet:coedge#1".into()),
            CoedgeId("synthetic:sheet:coedge#2".into()),
        ];
        let curve_id = CurveId("synthetic:sheet:curve#nurbs".into());
        let positions = [
            Point3::new(0.0, 0.0, 20.0),
            Point3::new(10.0, 0.0, 20.0),
            Point3::new(0.0, 10.0, 20.0),
        ];

        ir.model.bodies.push(Body {
            id: body_id.clone(),
            kind: BodyKind::Sheet,
            regions: vec![region_id.clone()],
            transform: None,
            name: None,
            color: None,
            visible: None,
        });
        ir.model.regions.push(Region {
            id: region_id.clone(),
            body: body_id.clone(),
            shells: vec![shell_id.clone()],
        });
        ir.model.shells.push(Shell {
            id: shell_id.clone(),
            region: region_id,
            faces: vec![face_id.clone()],
            wire_edges: Vec::new(),
            free_vertices: Vec::new(),
        });
        ir.model.faces.push(Face {
            id: face_id.clone(),
            shell: shell_id,
            surface: surface_id.clone(),
            sense: Sense::Forward,
            loops: vec![loop_id.clone()],
            name: None,
            color: None,
            tolerance: None,
        });
        ir.model.loops.push(Loop {
            id: loop_id.clone(),
            face: face_id,
            boundary_role: LoopBoundaryRole::Outer,
            coedges: coedge_ids.to_vec(),
            vertex_uses: Vec::new(),
        });
        for index in 0..3 {
            ir.model.coedges.push(Coedge {
                id: coedge_ids[index].clone(),
                owner_loop: loop_id.clone(),
                edge: edge_ids[index].clone(),
                next: coedge_ids[(index + 1) % 3].clone(),
                previous: coedge_ids[(index + 2) % 3].clone(),
                radial_next: coedge_ids[index].clone(),
                sense: Sense::Forward,
                pcurves: Vec::new(),
                use_curve: None,
                use_curve_parameter_range: None,
            });
            ir.model.edges.push(Edge {
                id: edge_ids[index].clone(),
                curve: (index == 0).then(|| curve_id.clone()),
                start: vertex_ids[index].clone(),
                end: vertex_ids[(index + 1) % 3].clone(),
                param_range: None,
                tolerance: None,
            });
            ir.model.vertices.push(Vertex {
                id: vertex_ids[index].clone(),
                point: point_ids[index].clone(),
                tolerance: None,
            });
            ir.model.points.push(Point {
                id: point_ids[index].clone(),
                position: positions[index],
                source_object: None,
            });
        }
        ir.model.curves.push(Curve {
            id: curve_id,
            geometry: CurveGeometry::Nurbs(NurbsCurve {
                degree: 1,
                knots: vec![0.0, 0.0, 1.0, 1.0],
                control_points: vec![positions[0], positions[1]],
                weights: None,
                periodic: false,
            }),
            source_object: None,
        });
        ir.model.surfaces.push(Surface {
            id: surface_id,
            geometry: SurfaceGeometry::Nurbs(NurbsSurface {
                u_degree: 1,
                v_degree: 1,
                u_knots: vec![0.0, 0.0, 1.0, 1.0],
                v_knots: vec![0.0, 0.0, 1.0, 1.0],
                u_count: 2,
                v_count: 2,
                control_points: vec![
                    positions[0],
                    positions[2],
                    positions[1],
                    Point3::new(10.0, 10.0, 20.0),
                ],
                weights: Some(vec![1.0; 4]),
                u_periodic: false,
                v_periodic: false,
            }),
            source_object: None,
        });
        body_id
    }

    fn encoded_controlled_composite() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut ir = cadmpeg_ir::examples::unit_cube();
        ir.model.bodies[0].name = None;
        ir.model.faces.iter_mut().for_each(|face| face.name = None);
        ir.model
            .edges
            .iter_mut()
            .for_each(|edge| edge.param_range = None);
        let solid_id = ir.model.bodies[0].id.clone();
        let sheet_id = append_open_nurbs_sheet(&mut ir);
        ir.model.configurations = vec![
            DesignConfiguration {
                id: ConfigurationId("synthetic:model:configuration#base".into()),
                ordinal: 0,
                active: false.into(),
                source_index: None,
                name: "Base".into(),
                material: None,
                properties: BTreeMap::new(),
                bodies: ConfigurationBodies::Resolved(vec![solid_id.clone()]),
                parameter_values: BTreeMap::new(),
                suppressed_features: Vec::new(),
                parameter_overrides: BTreeMap::new(),
                feature_states: BTreeMap::new(),
                native_ref: None,
            },
            DesignConfiguration {
                id: ConfigurationId("synthetic:model:configuration#derived".into()),
                ordinal: 1,
                active: true.into(),
                source_index: None,
                name: "Derived".into(),
                material: None,
                properties: BTreeMap::new(),
                bodies: ConfigurationBodies::Resolved(vec![solid_id, sheet_id]),
                parameter_values: BTreeMap::new(),
                suppressed_features: Vec::new(),
                parameter_overrides: BTreeMap::new(),
                feature_states: BTreeMap::new(),
                native_ref: None,
            },
        ];
        ir.finalize();

        let mut bytes = Vec::new();
        SldprtCodec
            .plan(EncodeInput {
                ir: &ir,
                fidelity: None,
            })?
            .write_to(&mut bytes)?;
        Ok(bytes)
    }

    #[test]
    fn source_less_cube_maps_topology_and_exact_stream_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let bytes = encoded_cube()?;
        let limits = ResourceLimits::desktop();
        let first = decode(&bytes, Some("cube.SLDPRT"), SourceInputKind::Bytes, &limits);
        let second = decode(&bytes, Some("cube.SLDPRT"), SourceInputKind::Bytes, &limits);
        assert_eq!(first, second);
        assert!(matches!(
            first.status,
            GeometryStatus::Decoded | GeometryStatus::Partial
        ));

        let geometry = first.geometry.as_ref().ok_or("geometry document missing")?;
        assert!(geometry.fidelity.geometry_transferred);
        assert_eq!(geometry.model.bodies.len(), 1);
        assert_eq!(geometry.model.faces.len(), 6);
        assert_eq!(geometry.model.edges.len(), 12);
        assert_eq!(geometry.model.vertices.len(), 8);
        assert_eq!(geometry.model.points.len(), 8);
        assert!(geometry.model.constructions.is_empty());
        assert_eq!(geometry.topology_metrics.len(), 1);
        assert_eq!(geometry.topology_metrics[0].euler_characteristic, Some(2));
        assert_eq!(geometry.topology_metrics[0].faces, 6);
        assert_eq!(geometry.topology_metrics[0].edges, 12);
        assert_eq!(geometry.topology_metrics[0].vertices, 8);
        let coverage = geometry.fidelity.byte_coverage;
        assert_eq!(
            coverage.partition_status,
            GeometryBytePartitionStatus::Incomplete
        );
        assert!(coverage.located_entity_count > 0);
        assert!(coverage.unique_location_count > 0);
        assert!(coverage.unique_location_count <= coverage.located_entity_count);
        assert_eq!(coverage.classified_active_bytes, 0);
        assert_eq!(
            coverage.unclassified_active_bytes,
            coverage.partition_domain_bytes
        );
        assert!(coverage.partition_domain_bytes > 0);
        assert_eq!(coverage.typed_bytes, None);
        assert_eq!(coverage.uninterpreted_bytes, None);

        assert!(!geometry.fidelity.byte_domains.is_empty());
        assert_eq!(
            coverage.partition_domain_bytes,
            geometry
                .fidelity
                .byte_domains
                .iter()
                .map(|domain| domain.byte_len)
                .sum::<u64>()
        );
        for domain in &geometry.fidelity.byte_domains {
            assert_eq!(domain.offset_basis, GeometryByteOffsetBasis::ParasolidBody);
            assert_eq!(
                domain.body_offset.saturating_add(domain.byte_len),
                domain.stream_byte_len
            );
            assert_eq!(domain.sha256.len(), 64);
            assert_eq!(domain.stream_sha256.len(), 64);
        }

        let active = geometry
            .source_streams
            .iter()
            .find(|stream| {
                stream.role == GeometryStreamRole::ParasolidPartition
                    && stream.selection == GeometryStreamSelection::Active
            })
            .ok_or("active Parasolid partition missing")?;
        let extracted =
            crate::extract_bytes(&bytes, &active.entry_id, ExtractionMode::Decoded, &limits);
        assert_eq!(extracted.result.status, ExtractionStatus::Extracted);
        assert_eq!(extracted.result.byte_len, active.decoded_size);
        assert_eq!(extracted.result.sha256, active.decoded_sha256);
        assert!(
            geometry
                .fidelity
                .byte_domains
                .iter()
                .all(|domain| domain.container_entry_id == active.entry_id)
        );
        assert_eq!(
            extracted.data.as_deref().map(super::sha256_hex),
            active.decoded_sha256
        );
        Ok(())
    }

    #[test]
    fn source_less_composite_maps_multibody_sheet_nurbs_and_configuration_membership()
    -> Result<(), Box<dyn std::error::Error>> {
        let bytes = encoded_controlled_composite()?;
        let result = decode(
            &bytes,
            Some("controlled-composite.SLDPRT"),
            SourceInputKind::Bytes,
            &ResourceLimits::desktop(),
        );
        assert!(matches!(
            result.status,
            GeometryStatus::Decoded | GeometryStatus::Partial
        ));
        let geometry = result.geometry.ok_or("geometry document missing")?;

        let base = geometry
            .configurations
            .iter()
            .find(|configuration| configuration.name.as_deref() == Some("Base"))
            .ok_or("base configuration missing")?;
        let derived = geometry
            .configurations
            .iter()
            .find(|configuration| configuration.name.as_deref() == Some("Derived"))
            .ok_or("derived configuration missing")?;
        assert_eq!(base.active, Some(false));
        assert_eq!(derived.active, Some(true));
        assert_eq!(base.body_ids.as_ref().map(Vec::len), Some(1));
        assert_eq!(derived.body_ids.as_ref().map(Vec::len), Some(2));

        let sheet = geometry
            .model
            .bodies
            .iter()
            .find(|body| body.kind == "sheet")
            .ok_or("sheet body missing")?;
        let sheet_metrics = geometry
            .topology_metrics
            .iter()
            .find(|metrics| metrics.body_id == sheet.id)
            .ok_or("sheet metrics missing")?;
        assert_eq!(sheet_metrics.faces, 1);
        assert_eq!(sheet_metrics.edges, 3);
        assert_eq!(sheet_metrics.vertices, 3);
        assert_eq!(sheet_metrics.euler_characteristic, Some(1));

        let nurbs_curve = geometry
            .model
            .carriers
            .iter()
            .find(|carrier| {
                carrier.domain == GeometryCarrierDomain::Curve && carrier.kind == "nurbs"
            })
            .ok_or("NURBS curve missing")?;
        assert_eq!(nurbs_curve.definition["degree"], 1);
        assert_eq!(
            nurbs_curve.definition["control_points"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
        let nurbs_surface = geometry
            .model
            .carriers
            .iter()
            .find(|carrier| {
                carrier.domain == GeometryCarrierDomain::Surface && carrier.kind == "nurbs"
            })
            .ok_or("NURBS surface missing")?;
        assert_eq!(nurbs_surface.definition["u_degree"], 1);
        assert_eq!(nurbs_surface.definition["v_degree"], 1);
        assert_eq!(
            nurbs_surface.definition["weights"].as_array().map(Vec::len),
            Some(4)
        );
        Ok(())
    }

    #[test]
    fn missing_entity_annotations_remain_unknown() {
        let annotations = cadmpeg_ir::Annotations::default();
        let provenance = super::provenance("missing", &annotations);
        assert_eq!(
            provenance.exactness,
            sldkit_core::GeometryExactness::Unknown
        );
        assert!(provenance.stream.is_none());
        assert!(provenance.offset.is_none());
    }

    #[test]
    fn procedural_carrier_keeps_its_construction_link() -> Result<(), Box<dyn std::error::Error>> {
        let mut ir = cadmpeg_ir::examples::unit_cube();
        let produced_surface = ir.model.surfaces[0].id.clone();
        let support_surface = ir.model.surfaces[1].id.clone();
        let construction_id = ProceduralSurfaceId("construction:test".to_owned());
        ir.model.surfaces[0].geometry = SurfaceGeometry::Procedural {
            construction: construction_id.clone(),
        };
        ir.model.procedural_surfaces.push(ProceduralSurface {
            id: construction_id.clone(),
            surface: produced_surface.clone(),
            definition: ProceduralSurfaceDefinition::Compound {
                parameters: vec![1.0],
                components: vec![support_surface],
            },
            cache_fit_tolerance: Some(1.0e-6),
            record_bounds: Some([Some(0.0), Some(1.0), None, None]),
        });

        let model = super::map_model(&ir, &cadmpeg_ir::SourceFidelity::default())?;
        let carrier = model
            .carriers
            .iter()
            .find(|item| item.id == produced_surface.0)
            .ok_or("procedural carrier missing")?;
        assert_eq!(carrier.kind, "procedural");
        assert_eq!(
            carrier
                .definition
                .get("construction")
                .and_then(|item| item.as_str()),
            Some(construction_id.0.as_str())
        );
        let construction = model
            .constructions
            .iter()
            .find(|item| item.id == construction_id.0)
            .ok_or("construction missing")?;
        assert_eq!(construction.produced_carrier_id, produced_surface.0);
        assert_eq!(construction.cache_fit_tolerance, Some(1.0e-6));
        assert_eq!(
            construction.record_bounds,
            Some([Some(0.0), Some(1.0), None, None])
        );
        Ok(())
    }
}
