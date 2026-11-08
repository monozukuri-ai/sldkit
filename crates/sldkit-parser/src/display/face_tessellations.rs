//! `FaceTessellations` streams of SOLIDWORKS 2023 and later assemblies.
//!
//! These assemblies keep `Contents/DisplayLists` for the environment only and
//! save the display tables of each referenced part once, in a section of a
//! `FaceTessellations/<stream>` entry listed by `FaceTessellations/Directory`.
//!
//! The directory is a 16-byte header (`u32` format, `u32` last stream number,
//! `u32` record count, `u32` tree index of the first record) followed by one
//! record per component occurrence: the MFC string of the occurrence path
//! (`<name>-<n>@<assembly>`, nested occurrences joined with `/`), a `u32`
//! model stamp, two zero `u32`, the 8-byte id of the owner this occurrence
//! repeats (zero when it owns its tables), the record's own 8-byte id, the MFC
//! string of the stream holding its tables (empty otherwise), the `u32` section
//! ordinal (`0xffff_ffff` without a section), the `u32` body count, 12 bytes
//! per body, and the `u32` tree index of the next record.
//!
//! A stream holds its sections in ordinal order: `u32` ordinal, the owner's
//! 8-byte id, `u32` face count, the faces, and one trailing `u32`. A face is
//! the `DisplayLists` face header (compact or extended), the six-channel table
//! of [`tables::parse_table`], and a trailer with the table bounds as `f64`,
//! flags, and the framed persistent-surface references.

use std::collections::{BTreeMap, HashMap};

use sldkit_core::{
    Diagnostic, DiagnosticKind, DiagnosticSeverity, DisplayMeshInstance, DisplayMeshStream,
    InventoryEntry,
};

use super::tables::{self, Face, find_all, u16_le_at, u32_le_at};
use super::{Packer, diagnostic, to_u64};

const DIRECTORY_HEADER_LEN: usize = 16;
const DECLARED_RECORDS_AT: usize = 8;
const FIRST_TREE_INDEX_AT: usize = 12;
const ID_LEN: usize = 8;
/// Model stamp, two zero words, reference id, own id.
const RECORD_FIXED_LEN: usize = 28;
const REFERENCE_ID_AT: usize = 12;
const OWN_ID_AT: usize = 20;
const BODY_LEN: usize = 12;
const NO_SECTION: u32 = u32::MAX;
const UTF16_MARKER: [u8; 3] = [0xff, 0xfe, 0xff];
/// Descriptor of the strips channel that opens every face table.
const STRIPS_CHANNEL_PREFIX: [u8; 12] = [4, 0, 0, 0, 8, 0, 0, 0, 2, 0, 0, 0];
/// Section ordinal before the id, then the id and the face count.
const SECTION_HEADER_LEN: usize = ID_LEN + 4;

const DIRECTORY_PATH: &str = "FaceTessellations/Directory";
const STREAM_PREFIX: &str = "FaceTessellations/";

/// `FaceTessellations/<stream>` entries other than the directory.
pub(super) fn is_stream_path(path: &str) -> bool {
    stream_name(path).is_some()
}

pub(super) fn is_directory_path(path: &str) -> bool {
    path.trim_start_matches('/') == DIRECTORY_PATH
}

fn stream_name(path: &str) -> Option<&str> {
    path.trim_start_matches('/')
        .strip_prefix(STREAM_PREFIX)
        .filter(|name| !name.is_empty() && *name != "Directory" && !name.contains('/'))
}

/// One directory record: a component occurrence of the assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectoryRecord {
    pub(super) tree_index: u32,
    pub(super) path: String,
    pub(super) reference_id: [u8; ID_LEN],
    pub(super) id: [u8; ID_LEN],
    /// Stream name and section ordinal of the tables this occurrence owns.
    pub(super) stream: Option<String>,
    pub(super) section: Option<u32>,
    pub(super) body_count: u32,
}

impl DirectoryRecord {
    pub(super) fn owns_tables(&self) -> bool {
        self.stream.is_some() && self.section.is_some()
    }

    pub(super) fn repeats(&self) -> bool {
        self.reference_id != [0; ID_LEN]
    }

    /// `<name>` and `<n>` of the last path segment.
    pub(super) fn leaf(&self) -> (String, Option<u32>) {
        let segment = self.path.rsplit('/').next().unwrap_or(&self.path);
        let occurrence = segment.rsplit_once('@').map_or(segment, |(head, _)| head);
        match occurrence.rsplit_once('-') {
            Some((name, number)) if !name.is_empty() => match number.parse::<u32>() {
                Ok(number) => (name.to_owned(), Some(number)),
                Err(_) => (occurrence.to_owned(), None),
            },
            _ => (occurrence.to_owned(), None),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Directory {
    pub(super) records: Vec<DirectoryRecord>,
    /// Record count the header declares.
    pub(super) declared_records: u32,
    /// A record ran past the end of the stream.
    pub(super) truncated: bool,
}

fn mfc_string(payload: &[u8], at: usize) -> Option<(String, usize)> {
    if payload.get(at..at + UTF16_MARKER.len())? != UTF16_MARKER {
        return None;
    }
    let mut cursor = at + UTF16_MARKER.len();
    let mut count = usize::from(*payload.get(cursor)?);
    cursor += 1;
    if count == 0xff {
        count = usize::from(u16_le_at(payload, cursor)?);
        cursor += 2;
        if count == 0xffff {
            count = usize::try_from(u32_le_at(payload, cursor)?).ok()?;
            cursor += 4;
        }
    }
    let end = cursor.checked_add(count.checked_mul(2)?)?;
    let units = payload
        .get(cursor..end)?
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    Some((String::from_utf16(&units).ok()?, end))
}

fn id_at(payload: &[u8], at: usize) -> Option<[u8; ID_LEN]> {
    let mut id = [0; ID_LEN];
    id.copy_from_slice(payload.get(at..at + ID_LEN)?);
    Some(id)
}

/// One record at `at`, and the offset after its body list.
fn parse_record(payload: &[u8], at: usize, tree_index: u32) -> Option<(DirectoryRecord, usize)> {
    let (path, after_path) = mfc_string(payload, at)?;
    let reference_id = id_at(payload, after_path + REFERENCE_ID_AT)?;
    let id = id_at(payload, after_path + OWN_ID_AT)?;
    let (stream, after_stream) = mfc_string(payload, after_path + RECORD_FIXED_LEN)?;
    let section = u32_le_at(payload, after_stream)?;
    let body_count = u32_le_at(payload, after_stream + 4)?;
    let bodies_len = usize::try_from(body_count).ok()?.checked_mul(BODY_LEN)?;
    let end = after_stream.checked_add(8)?.checked_add(bodies_len)?;
    (end <= payload.len()).then_some((
        DirectoryRecord {
            tree_index,
            path,
            reference_id,
            id,
            stream: (!stream.is_empty()).then_some(stream),
            section: (section != NO_SECTION).then_some(section),
            body_count,
        },
        end,
    ))
}

/// Parse the directory. `None` when the header or the first record is absent.
pub(super) fn parse_directory(payload: &[u8]) -> Option<Directory> {
    let declared_records = u32_le_at(payload, DECLARED_RECORDS_AT)?;
    let mut tree_index = u32_le_at(payload, FIRST_TREE_INDEX_AT)?;
    let mut cursor = DIRECTORY_HEADER_LEN;
    let mut records = Vec::new();
    let mut truncated = false;
    while cursor < payload.len() {
        let Some((record, end)) = parse_record(payload, cursor, tree_index) else {
            truncated = true;
            break;
        };
        records.push(record);
        cursor = end;
        match u32_le_at(payload, cursor) {
            Some(next)
                if payload.get(cursor + 4..cursor + 4 + UTF16_MARKER.len())
                    == Some(UTF16_MARKER.as_slice()) =>
            {
                tree_index = next;
                cursor += 4;
            }
            _ => break,
        }
    }
    (!records.is_empty()).then_some(Directory {
        records,
        declared_records,
        truncated,
    })
}

/// One owner's section inside a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Section {
    /// Index into the directory records.
    pub(super) record: usize,
    /// Offset of the owner id; the faces follow the face count after it.
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) declared_faces: u32,
}

/// Locate the sections of `stream` from the records that name it.
///
/// A section starts where the record's id is preceded by its ordinal. Records
/// whose section is not found are returned as the second value.
pub(super) fn sections(
    payload: &[u8],
    records: &[DirectoryRecord],
    stream: &str,
) -> (Vec<Section>, Vec<usize>) {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if record.stream.as_deref() != Some(stream) {
            continue;
        }
        let Some(ordinal) = record.section else {
            missing.push(index);
            continue;
        };
        let candidates = find_all(payload, &record.id);
        let start = candidates
            .iter()
            .copied()
            .find(|at| {
                at.checked_sub(4)
                    .and_then(|before| u32_le_at(payload, before))
                    == Some(ordinal)
            })
            .or_else(|| (candidates.len() == 1).then(|| candidates[0]));
        match start.and_then(|start| Some((start, u32_le_at(payload, start + ID_LEN)?))) {
            Some((start, declared_faces)) => found.push(Section {
                record: index,
                start,
                end: payload.len(),
                declared_faces,
            }),
            None => missing.push(index),
        }
    }
    found.sort_by_key(|section| section.start);
    let starts = found
        .iter()
        .map(|section| section.start)
        .collect::<Vec<_>>();
    for (section, next) in found.iter_mut().zip(starts.iter().skip(1)) {
        section.end = *next;
    }
    (found, missing)
}

/// Offset of the compact or extended face header whose counts match `table`.
fn face_header(payload: &[u8], table_start: usize, table: &tables::Table) -> Option<usize> {
    [tables::COMPACT_HEADER_LEN, tables::EXTENDED_HEADER_LEN]
        .into_iter()
        .find_map(|width| {
            let header = table_start.checked_sub(width)?;
            (tables::header_len(payload, header) == width
                && tables::header_agrees(payload, header, table))
            .then_some(header)
        })
}

/// Validated faces between `start` and `end`, in byte order.
///
/// Tables are not aligned, so every strips descriptor is tried; a table counts
/// only when a face header stating its triangle and strip counts precedes it.
pub(super) fn faces(payload: &[u8], start: usize, end: usize) -> Vec<Face> {
    let end = end.min(payload.len());
    let Some(window) = payload.get(start..end) else {
        return Vec::new();
    };
    let mut faces: Vec<Face> = Vec::new();
    let mut cursor = start;
    for relative in find_all(window, &STRIPS_CHANNEL_PREFIX) {
        let at = start + relative;
        if at < cursor {
            continue;
        }
        let Some((table, table_end)) = tables::parse_table(payload, at) else {
            continue;
        };
        if table_end > end {
            continue;
        }
        let Some(header) = face_header(payload, at, &table) else {
            continue;
        };
        if let Some(previous) = faces.last_mut() {
            previous.metadata_end = header;
        }
        faces.push(Face {
            table,
            start: at,
            end: table_end,
            metadata_start: table_end,
            metadata_end: end,
            surface_references: Vec::new(),
        });
        cursor = table_end;
    }
    for face in &mut faces {
        face.surface_references =
            tables::surface_references(payload, face.metadata_start, face.metadata_end);
    }
    faces
}

fn instance_from_record(record: &DirectoryRecord) -> DisplayMeshInstance {
    let (name, number) = record.leaf();
    DisplayMeshInstance {
        path: record.path.clone(),
        name,
        number,
        tree_index: record.tree_index,
        tables_from: None,
        stream_index: None,
        section_index: record.section,
        declared_face_count: None,
        face_count: 0,
        body_count: record.body_count,
    }
}

/// What the `FaceTessellations` entries contributed.
#[derive(Debug, Default)]
pub(super) struct Outcome {
    pub(super) instances: Vec<DisplayMeshInstance>,
    /// Tables packed from these streams.
    pub(super) parts: u64,
    /// A directory, section, or reference did not resolve; the result is partial.
    pub(super) incomplete: bool,
}

fn warning(code: &'static str, message: &'static str) -> Diagnostic {
    diagnostic(
        code,
        DiagnosticSeverity::Warning,
        DiagnosticKind::Unresolved,
        message,
    )
}

/// Pack every face of the `FaceTessellations` streams and list the occurrences.
#[allow(clippy::too_many_lines)]
pub(super) fn decode(
    entries: &[&InventoryEntry],
    directory: Option<&[u8]>,
    decoded: &BTreeMap<String, Vec<u8>>,
    packer: &mut Packer,
    streams: &mut Vec<DisplayMeshStream>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Outcome {
    let directory = directory.and_then(parse_directory);
    let mut incomplete = false;
    match &directory {
        None => {
            diagnostics.push(warning(
                "display.face_tessellations_directory_unreadable",
                "FaceTessellations streams have no readable directory; their tables are returned without occurrences",
            ));
            incomplete = true;
        }
        Some(directory)
            if directory.truncated
                || usize::try_from(directory.declared_records).ok()
                    != Some(directory.records.len()) =>
        {
            diagnostics.push(
                warning(
                    "display.face_tessellations_directory_incomplete",
                    "the FaceTessellations directory lists fewer occurrences than it declares",
                )
                .with_detail("declared", directory.declared_records.to_string())
                .with_detail("read", directory.records.len().to_string()),
            );
            incomplete = true;
        }
        Some(_) => {}
    }
    let records = directory
        .as_ref()
        .map_or(&[][..], |directory| directory.records.as_slice());
    let mut instances = records.iter().map(instance_from_record).collect::<Vec<_>>();
    let owners = records
        .iter()
        .enumerate()
        .filter(|(_, record)| record.owns_tables())
        .map(|(index, record)| (record.id, index))
        .collect::<HashMap<_, _>>();

    let mut parts = 0u64;
    'streams: for entry in entries {
        let Some(payload) = decoded.get(&entry.id).filter(|payload| !payload.is_empty()) else {
            continue;
        };
        let path = entry.path.clone().unwrap_or_default();
        let name = stream_name(&path).unwrap_or(&path);
        let Ok(stream_index) = u32::try_from(streams.len()) else {
            break;
        };
        let (sections, missing) = sections(payload, records, name);
        for record in missing {
            diagnostics.push(
                warning(
                    "display.face_tessellations_section_missing",
                    "an occurrence names this stream but its section was not found",
                )
                .with_detail("stream_path", path.clone())
                .with_detail(
                    "occurrence",
                    records
                        .get(record)
                        .map(|r| r.path.clone())
                        .unwrap_or_default(),
                ),
            );
            incomplete = true;
        }
        let first = sections
            .first()
            .map_or(payload.len(), |section| section.start);
        let mut ranges = vec![(None, 0usize, first)];
        ranges.extend(sections.iter().map(|section| {
            (
                Some(section),
                section.start + SECTION_HEADER_LEN,
                section.end,
            )
        }));
        let mut table_index = 0u32;
        let mut stream_parts = 0u64;
        let mut stopped = false;
        for (section, start, end) in ranges {
            let found = faces(payload, start, end);
            let found_count = u32::try_from(found.len()).unwrap_or(u32::MAX);
            let instance_index = section.and_then(|section| u32::try_from(section.record).ok());
            if let Some(section) = section {
                if let Some(instance) = instances.get_mut(section.record) {
                    instance.stream_index = Some(stream_index);
                    instance.declared_face_count = Some(section.declared_faces);
                    instance.face_count = found_count;
                    instance.tables_from = instance_index;
                }
                if found_count != section.declared_faces {
                    diagnostics.push(
                        warning(
                            "display.face_tessellations_section_incomplete",
                            "a section yielded fewer validated tables than it declares",
                        )
                        .with_detail("stream_path", path.clone())
                        .with_detail(
                            "occurrence",
                            records
                                .get(section.record)
                                .map(|r| r.path.clone())
                                .unwrap_or_default(),
                        )
                        .with_detail("declared", section.declared_faces.to_string())
                        .with_detail("found", found_count.to_string()),
                    );
                    incomplete = true;
                }
            } else if !found.is_empty() {
                diagnostics.push(
                    warning(
                        "display.face_tessellations_tables_unassigned",
                        "tables before the first section belong to no occurrence",
                    )
                    .with_detail("stream_path", path.clone())
                    .with_detail("tables", found_count.to_string()),
                );
                incomplete = true;
            }
            for face in &found {
                if !packer.push(stream_index, table_index, face, None, instance_index) {
                    stopped = true;
                    break;
                }
                table_index = table_index.saturating_add(1);
                stream_parts += 1;
            }
            if stopped {
                break;
            }
        }
        parts += stream_parts;
        streams.push(DisplayMeshStream {
            entry_id: entry.id.clone(),
            stream_path: path,
            decoded_size: to_u64(payload.len()),
            decoded_sha256: entry.decoded_sha256.clone(),
            part_count: stream_parts,
            trailing_names: Vec::new(),
        });
        if stopped {
            break 'streams;
        }
    }

    let mut unresolved = 0u64;
    let mut repeats = 0u64;
    for (index, record) in records.iter().enumerate() {
        if !record.repeats() {
            continue;
        }
        let owner = owners.get(&record.reference_id).copied().filter(|owner| {
            instances
                .get(*owner)
                .is_some_and(|instance| instance.tables_from.is_some())
        });
        match (
            owner.and_then(|owner| u32::try_from(owner).ok()),
            instances.get_mut(index),
        ) {
            (Some(owner), Some(instance)) => {
                instance.tables_from = Some(owner);
                repeats += 1;
            }
            _ => unresolved += 1,
        }
    }
    if unresolved > 0 {
        diagnostics.push(
            warning(
                "display.face_tessellations_reference_unresolved",
                "occurrences repeat an owner whose tables were not found",
            )
            .with_detail("occurrences", unresolved.to_string()),
        );
        incomplete = true;
    }
    if !instances.is_empty() {
        let owning = instances
            .iter()
            .filter(|instance| instance.stream_index.is_some())
            .count();
        let without = instances
            .iter()
            .filter(|instance| instance.tables_from.is_none())
            .count();
        diagnostics.push(
            diagnostic(
                "display.face_tessellations_directory",
                DiagnosticSeverity::Info,
                DiagnosticKind::Preserved,
                "occurrences come from the FaceTessellations directory; tables stay in each referenced part's coordinates",
            )
            .with_detail("occurrences", instances.len().to_string())
            .with_detail("owning_tables", owning.to_string())
            .with_detail("repeating", repeats.to_string())
            .with_detail("without_tables", without.to_string()),
        );
    }
    Outcome {
        instances,
        parts,
        incomplete,
    }
}
