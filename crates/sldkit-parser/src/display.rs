//! Saved display meshes (`DisplayLists`) without B-Rep decoding.
//!
//! This capability reads sldkit's own container entries, packs the validated
//! face tables into shared vertex and index arrays, derives face-outline edges,
//! and reports stream-level coverage. It never runs the Parasolid decoder, so
//! B-Rep failures, byte-ledger findings, and B-Rep cost do not affect it.

mod appearance;
mod tables;

use std::collections::{BTreeSet, HashMap};

use sldkit_container::decode_selected_bytes;
use sldkit_core::{
    Diagnostic, DiagnosticKind, DiagnosticSeverity, DisplayMeshBounds, DisplayMeshBuffers,
    DisplayMeshDocument, DisplayMeshPart, DisplayMeshResult, DisplayMeshStatus, DisplayMeshStream,
    DisplaySurfaceReference, DocumentKind, Envelope, InventoryEntryState, InventoryResult,
    InventoryStatus, ResourceLimits, SourceInfo, SourceInputKind,
};

const VISUAL_STATES_PATH: &str = "ThirdPtyStore/VisualStates";
const METRES_TO_MILLIMETRES: f32 = 1000.0;
/// Packed `f32`/`u32` arrays stay well below the decoded-byte budget.
const BYTES_PER_VERTEX_BUDGET: u64 = 64;

/// `Contents/DisplayLists` (modern) or `/Contents/DisplayLists__ZLB` (OLE2).
fn is_display_path(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with("DisplayLists"))
}

fn is_visual_states_path(path: &str) -> bool {
    path.trim_start_matches('/') == VISUAL_STATES_PATH
}

fn diagnostic(
    code: &'static str,
    severity: DiagnosticSeverity,
    kind: DiagnosticKind,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::new(code, severity, kind, message)
}

fn without_mesh(
    status: DisplayMeshStatus,
    diagnostics: Vec<Diagnostic>,
) -> (DisplayMeshResult, DisplayMeshBuffers) {
    (
        DisplayMeshResult {
            status,
            mesh: None,
            diagnostics,
        },
        DisplayMeshBuffers::default(),
    )
}

fn inspection_failure(inspection: &InventoryResult) -> Option<DisplayMeshStatus> {
    match inspection.status {
        InventoryStatus::Rejected => Some(DisplayMeshStatus::Rejected),
        InventoryStatus::Malformed => Some(DisplayMeshStatus::Malformed),
        InventoryStatus::Unsupported if inspection.inventory.is_none() => {
            Some(DisplayMeshStatus::Unsupported)
        }
        InventoryStatus::Complete | InventoryStatus::Partial | InventoryStatus::Unsupported => None,
    }
}

/// Document kind and internal version from the container's own semantics.
struct DocumentFacts {
    kind: DocumentKind,
    internal_version: Option<u64>,
    diagnostics: Vec<Diagnostic>,
    rejected: bool,
}

fn document_facts(
    data: &[u8],
    inventory: &sldkit_core::ContainerInventory,
    filename: Option<&str>,
    limits: &ResourceLimits,
) -> DocumentFacts {
    let hint = super::document_kind_hint(filename);
    if inventory.envelope == Envelope::Ole2Cfb {
        let facts = super::legacy_semantics::decode(data, inventory, &hint, limits);
        DocumentFacts {
            kind: facts.document_kind.value,
            internal_version: facts.internal_version.map(|value| value.value),
            diagnostics: facts.diagnostics,
            rejected: facts.rejected,
        }
    } else {
        let facts = super::modern_semantics::decode(data, inventory, &hint, limits);
        DocumentFacts {
            kind: facts.document_kind.value,
            internal_version: facts.internal_version.map(|value| value.value),
            diagnostics: facts.diagnostics,
            rejected: facts.rejected,
        }
    }
}

/// Merge feature colors from every `VisualStates` payload; disagreement drops the source.
fn merged_feature_colors<'a>(payloads: impl Iterator<Item = &'a [u8]>) -> HashMap<u32, u32> {
    let mut merged = HashMap::<u32, Option<u32>>::new();
    for payload in payloads {
        for (source, packed) in appearance::feature_colors(payload) {
            merged
                .entry(source)
                .and_modify(|existing| {
                    if *existing != Some(packed) {
                        *existing = None;
                    }
                })
                .or_insert(Some(packed));
        }
    }
    merged
        .into_iter()
        .filter_map(|(source, packed)| packed.map(|packed| (source, packed)))
        .collect()
}

/// Weld key: exact `f32` bits with negative zero folded into zero.
fn weld_key(position: &[f32; 3]) -> [u32; 3] {
    position.map(|value| {
        if value == 0.0 {
            0.0_f32.to_bits()
        } else {
            value.to_bits()
        }
    })
}

/// Edges used by exactly one triangle after welding coincident positions.
///
/// Display tables are face-local, so their outer and hole boundaries are the
/// face outlines. Output pairs use each welded position's first local index.
fn outline_edges(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<[u32; 2]> {
    let mut first_index = HashMap::<[u32; 3], u32>::with_capacity(positions.len());
    let canonical = positions
        .iter()
        .zip(0u32..)
        .map(|(position, index)| *first_index.entry(weld_key(position)).or_insert(index))
        .collect::<Vec<u32>>();
    let mut order = Vec::<[u32; 2]>::new();
    let mut uses = HashMap::<[u32; 2], u32>::new();
    for triangle in triangles {
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let (Some(&start), Some(&end)) = (
                usize::try_from(triangle[a])
                    .ok()
                    .and_then(|i| canonical.get(i)),
                usize::try_from(triangle[b])
                    .ok()
                    .and_then(|i| canonical.get(i)),
            ) else {
                continue;
            };
            if start == end {
                continue;
            }
            let key = [start.min(end), start.max(end)];
            let count = uses.entry(key).or_insert(0);
            if *count == 0 {
                order.push(key);
            }
            *count += 1;
        }
    }
    order
        .into_iter()
        .filter(|key| uses.get(key) == Some(&1))
        .collect()
}

/// Area-weighted vertex normals for a table that stored none.
fn derived_normals(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<[f32; 3]> {
    let mut sums = vec![[0.0_f64; 3]; positions.len()];
    for triangle in triangles {
        let corners = triangle.map(|index| usize::try_from(index).ok());
        let [Some(first), Some(second), Some(third)] = corners else {
            continue;
        };
        let (Some(origin), Some(along), Some(across)) = (
            positions.get(first),
            positions.get(second),
            positions.get(third),
        ) else {
            continue;
        };
        let side = [0, 1, 2].map(|axis| f64::from(along[axis]) - f64::from(origin[axis]));
        let other = [0, 1, 2].map(|axis| f64::from(across[axis]) - f64::from(origin[axis]));
        let cross = [
            side[1] * other[2] - side[2] * other[1],
            side[2] * other[0] - side[0] * other[2],
            side[0] * other[1] - side[1] * other[0],
        ];
        for corner in [first, second, third] {
            for (sum, component) in sums[corner].iter_mut().zip(cross) {
                *sum += component;
            }
        }
    }
    sums.into_iter()
        .map(|sum| {
            let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            if length > 0.0 && length.is_finite() {
                // f64 -> f32 narrowing is the documented packed representation.
                #[allow(clippy::cast_possible_truncation)]
                sum.map(|value| (value / length) as f32)
            } else {
                [0.0, 0.0, 1.0]
            }
        })
        .collect()
}

struct Packer {
    buffers: DisplayMeshBuffers,
    parts: Vec<DisplayMeshPart>,
    min: [f64; 3],
    max: [f64; 3],
    vertex_budget: u64,
    budget_exceeded: bool,
    derived_normal_parts: u64,
}

fn to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

impl Packer {
    fn new(limits: &ResourceLimits) -> Self {
        Self {
            buffers: DisplayMeshBuffers::default(),
            parts: Vec::new(),
            min: [f64::INFINITY; 3],
            max: [f64::NEG_INFINITY; 3],
            vertex_budget: (limits.max_total_uncompressed_bytes / BYTES_PER_VERTEX_BUDGET)
                .min(u64::from(u32::MAX)),
            budget_exceeded: false,
            derived_normal_parts: 0,
        }
    }

    fn vertex_count(&self) -> usize {
        self.buffers.positions.len() / 3
    }

    /// Append one face table. Returns `false` when the vertex budget stops packing.
    fn push(
        &mut self,
        stream_index: u32,
        table_index: u32,
        face: &tables::Face,
        appearance: Option<&appearance::Appearance>,
    ) -> bool {
        let table = &face.table;
        let Some(triangles) = table.triangles() else {
            return true;
        };
        let start = self.vertex_count();
        let next = to_u64(start).saturating_add(to_u64(table.positions.len()));
        if next > self.vertex_budget {
            self.budget_exceeded = true;
            return false;
        }
        let Ok(base) = u32::try_from(start) else {
            self.budget_exceeded = true;
            return false;
        };
        for position in &table.positions {
            let scaled = position.map(|value| value * METRES_TO_MILLIMETRES);
            for (axis, value) in scaled.iter().enumerate() {
                let value = f64::from(*value);
                self.min[axis] = self.min[axis].min(value);
                self.max[axis] = self.max[axis].max(value);
            }
            self.buffers.positions.extend(scaled);
        }
        let normals_derived = table.normals.is_empty();
        let normals = if normals_derived {
            self.derived_normal_parts += 1;
            derived_normals(&table.positions, &triangles)
        } else {
            table.normals.clone()
        };
        for normal in &normals {
            self.buffers.normals.extend(normal);
        }
        let triangle_start = self.buffers.indices.len() / 3;
        for triangle in &triangles {
            self.buffers
                .indices
                .extend(triangle.map(|index| index.saturating_add(base)));
        }
        let edge_start = self.buffers.edges.len() / 2;
        let edges = outline_edges(&table.positions, &triangles);
        for edge in &edges {
            self.buffers
                .edges
                .extend(edge.map(|index| index.saturating_add(base)));
        }
        self.parts.push(DisplayMeshPart {
            id: format!("displaylist:{stream_index}:{table_index}"),
            stream_index,
            table_index,
            table_offset: to_u64(face.start),
            vertex_start: to_u64(start),
            vertex_count: to_u64(table.positions.len()),
            triangle_start: to_u64(triangle_start),
            triangle_count: to_u64(triangles.len()),
            edge_start: to_u64(edge_start),
            edge_count: to_u64(edges.len()),
            strip_count: to_u64(table.strips.len()),
            normals_derived,
            color: appearance.map(|appearance| appearance.color),
            color_source: appearance.map(|appearance| appearance.source),
            appearance_name: appearance.and_then(|appearance| appearance.name.clone()),
            surface_references: face
                .surface_references
                .iter()
                .map(|reference| DisplaySurfaceReference {
                    identity: reference.identity.clone(),
                    feature_source_id: reference.feature_source_id,
                    local_surface_id: reference.local_surface_id,
                })
                .collect(),
        });
        true
    }

    fn bounds(&self) -> Option<DisplayMeshBounds> {
        (self.vertex_count() > 0).then_some(DisplayMeshBounds {
            min: self.min,
            max: self.max,
        })
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn decode(
    data: &[u8],
    filename: Option<&str>,
    input_kind: SourceInputKind,
    limits: &ResourceLimits,
) -> (DisplayMeshResult, DisplayMeshBuffers) {
    let inspection = super::inspect_bytes(data, filename, limits);
    if let Some(status) = inspection_failure(&inspection) {
        return without_mesh(status, inspection.diagnostics);
    }
    let Some(inventory) = inspection.inventory.as_ref() else {
        let mut diagnostics = inspection.diagnostics.clone();
        diagnostics.push(diagnostic(
            "display.container_unsupported",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unsupported,
            "display meshes require a recognized container inventory",
        ));
        return without_mesh(DisplayMeshStatus::Unsupported, diagnostics);
    };
    let mut diagnostics = inspection.diagnostics.clone();
    if !matches!(
        inventory.envelope,
        Envelope::ModernChunk | Envelope::Ole2Cfb
    ) {
        diagnostics.push(diagnostic(
            "display.envelope_unsupported",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unsupported,
            "display meshes are read from modern chunk and OLE2 containers only",
        ));
        return without_mesh(DisplayMeshStatus::Unsupported, diagnostics);
    }
    let document = document_facts(data, inventory, filename, limits);
    diagnostics.extend(document.diagnostics);
    if document.rejected {
        return without_mesh(DisplayMeshStatus::Rejected, diagnostics);
    }
    if document.kind == DocumentKind::Drawing {
        diagnostics.push(diagnostic(
            "display.document_kind_unsupported",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unsupported,
            "Drawing documents carry sheet previews, not model display meshes",
        ));
        return without_mesh(DisplayMeshStatus::Unsupported, diagnostics);
    }

    let display_entries = inventory
        .entries
        .iter()
        .filter(|entry| {
            entry.state == InventoryEntryState::Decoded
                && entry.path.as_deref().is_some_and(is_display_path)
        })
        .collect::<Vec<_>>();
    let visual_entries = inventory
        .entries
        .iter()
        .filter(|entry| {
            entry.state == InventoryEntryState::Decoded
                && entry.path.as_deref().is_some_and(is_visual_states_path)
        })
        .collect::<Vec<_>>();
    let wanted = display_entries
        .iter()
        .chain(visual_entries.iter())
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    let decoded = decode_selected_bytes(data, &wanted, limits).streams;
    let feature_colors = merged_feature_colors(
        visual_entries
            .iter()
            .filter_map(|entry| decoded.get(&entry.id).map(Vec::as_slice)),
    );

    let mut packer = Packer::new(limits);
    let mut streams = Vec::new();
    let mut unreadable = Vec::new();
    'streams: for entry in display_entries {
        let Some(payload) = decoded.get(&entry.id).filter(|payload| !payload.is_empty()) else {
            continue;
        };
        let path = entry.path.clone().unwrap_or_default();
        let classes = tables::class_intervals(payload);
        let faces = tables::faces(payload, &classes);
        let Ok(stream_index) = u32::try_from(streams.len()) else {
            break;
        };
        let tail = faces.iter().map(|face| face.end).max().unwrap_or(0);
        streams.push(DisplayMeshStream {
            entry_id: entry.id.clone(),
            stream_path: path.clone(),
            decoded_size: to_u64(payload.len()),
            decoded_sha256: entry.decoded_sha256.clone(),
            part_count: to_u64(faces.len()),
            trailing_names: if faces.is_empty() {
                Vec::new()
            } else {
                tables::trailing_names(payload, tail)
            },
        });
        if faces.is_empty() {
            unreadable.push(path);
            continue;
        }
        let appearances = appearance::resolve(payload, &classes, &faces, &feature_colors);
        for ((face, appearance), table_index) in faces.iter().zip(&appearances).zip(0u32..) {
            if !packer.push(stream_index, table_index, face, appearance.as_ref()) {
                break 'streams;
            }
        }
    }

    for path in &unreadable {
        let message = if path.ends_with("__Zip") {
            "legacy DisplayLists__Zip encoding is not decoded"
        } else {
            "display stream holds no validated face tessellation table"
        };
        diagnostics.push(
            diagnostic(
                "display.stream_unreadable",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Unsupported,
                message,
            )
            .with_detail("stream_path", path.clone()),
        );
    }
    let streams_with_parts = streams
        .iter()
        .filter(|stream| stream.part_count > 0)
        .count();
    if streams_with_parts > 1 {
        diagnostics.push(
            diagnostic(
                "display.multiple_streams",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Unresolved,
                "several display streams hold meshes; all are returned and configuration ownership is unresolved",
            )
            .with_detail("streams", streams_with_parts.to_string()),
        );
    }
    if packer.budget_exceeded {
        diagnostics.push(diagnostic(
            "display.vertex_budget_exceeded",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unsupported,
            "the vertex budget derived from the resource limits stopped packing; later tables are omitted",
        ));
    }
    if packer.derived_normal_parts > 0 {
        diagnostics.push(
            diagnostic(
                "display.normals_derived",
                DiagnosticSeverity::Info,
                DiagnosticKind::Inferred,
                "tables without stored normals received area-weighted vertex normals",
            )
            .with_detail("parts", packer.derived_normal_parts.to_string()),
        );
    }
    if document.kind == DocumentKind::Assembly && !packer.parts.is_empty() {
        diagnostics.push(diagnostic(
            "display.assembly_placement_unverified",
            DiagnosticSeverity::Info,
            DiagnosticKind::Inferred,
            "assembly display tables are returned in their stored coordinates; component placement is not decoded",
        ));
    }
    let status = if packer.parts.is_empty() {
        if streams.is_empty() {
            diagnostics.push(diagnostic(
                "display.no_display_data",
                DiagnosticSeverity::Warning,
                DiagnosticKind::Unsupported,
                "the document holds no non-empty DisplayLists stream",
            ));
        }
        DisplayMeshStatus::Unsupported
    } else if !unreadable.is_empty() || streams_with_parts > 1 || packer.budget_exceeded {
        DisplayMeshStatus::Partial
    } else {
        DisplayMeshStatus::Decoded
    };

    let bounds = packer.bounds();
    let vertex_count = to_u64(packer.vertex_count());
    let triangle_count = to_u64(packer.buffers.indices.len() / 3);
    let edge_count = to_u64(packer.buffers.edges.len() / 2);
    let source = SourceInfo {
        input_kind,
        label: filename.map(str::to_owned),
        byte_len: to_u64(data.len()),
        sha256: super::sha256_hex(data),
    };
    (
        DisplayMeshResult {
            status,
            mesh: Some(DisplayMeshDocument {
                source,
                envelope: inventory.envelope,
                document_kind: document.kind,
                internal_version: document.internal_version,
                length_unit: "millimeter".to_owned(),
                streams,
                parts: packer.parts,
                vertex_count,
                triangle_count,
                edge_count,
                bounds,
            }),
            diagnostics,
        },
        packer.buffers,
    )
}

#[cfg(test)]
mod tests;
