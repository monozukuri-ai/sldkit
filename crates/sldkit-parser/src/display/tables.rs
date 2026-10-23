// SPDX-License-Identifier: Apache-2.0
// Adopted from cadmpeg-codec-sldprt 0.5.3 `src/tessellation.rs` (Apache-2.0)
// and rewritten for sldkit-owned types; see THIRD_PARTY_NOTICES.md.
//! `DisplayLists` face-tessellation tables.
//!
//! A table holds six channel descriptors: strip lengths, `f32` positions in
//! metres, `f32` normals, and three auxiliary channels whose counts and values
//! must agree with the strips. Tables follow a `uoTempFaceTessData_c` MFC class
//! token or a reused old-class tag of a validated instance.

use std::collections::{HashMap, HashSet};

pub(super) const FACE_TESSELLATION_CLASS: &[u8] = b"uoTempFaceTessData_c";
const CLASS_MARKER: [u8; 4] = [0xff, 0xff, 0x01, 0x00];
const UTF16_MARKER: [u8; 3] = [0xff, 0xfe, 0xff];
const CHANNEL_COUNT: usize = 6;
const CHANNEL_HEADER_LEN: usize = 16;

// Face header offsets after the class token. The extended form carries a
// fixed 32-byte extension after the two counts.
const COMPACT_HEADER_LEN: usize = 8;
const EXTENDED_HEADER_LEN: usize = 40;
const HEADER_TRIANGLE_COUNT: usize = 0;
const HEADER_STRIP_COUNT: usize = 4;
const EXTENDED_FORM: usize = 8;
const EXTENDED_ZERO_AT_12: usize = 12;
const EXTENDED_ZERO_AT_16: usize = 16;
const EXTENDED_FORM_TOKEN: usize = 20;
const EXTENDED_ZERO_TAIL: usize = 24;

pub(super) fn u16_le_at(bytes: &[u8], at: usize) -> Option<u16> {
    let raw = bytes.get(at..at.checked_add(2)?)?;
    Some(u16::from_le_bytes([raw[0], raw[1]]))
}

pub(super) fn u32_le_at(bytes: &[u8], at: usize) -> Option<u32> {
    let raw = bytes.get(at..at.checked_add(4)?)?;
    Some(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

fn f32_le_at(bytes: &[u8], at: usize) -> Option<f32> {
    u32_le_at(bytes, at).map(f32::from_bits)
}

/// Byte offsets of every occurrence of `needle`.
pub(super) fn find_all(haystack: &[u8], needle: &[u8]) -> Vec<usize> {
    let Some(&first) = needle.first() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(relative) = haystack
        .get(at..)
        .and_then(|rest| rest.iter().position(|byte| *byte == first))
    {
        let offset = at + relative;
        if haystack.get(offset..offset + needle.len()) == Some(needle) {
            found.push(offset);
        }
        at = offset + 1;
    }
    found
}

/// Offset of the first occurrence of `needle`.
fn find_first(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// One MFC class declaration and the bytes until the next declaration.
#[derive(Debug, Clone)]
pub(super) struct ClassInterval {
    pub(super) name: String,
    pub(super) class_offset: usize,
    pub(super) content_start: usize,
    pub(super) content_end: usize,
}

pub(super) fn class_intervals(payload: &[u8]) -> Vec<ClassInterval> {
    let declarations = find_all(payload, &CLASS_MARKER)
        .into_iter()
        .filter_map(|offset| {
            let length = usize::from(u16_le_at(payload, offset + 4)?);
            if !(1..=128).contains(&length) {
                return None;
            }
            let name = payload.get(offset + 6..offset + 6 + length)?;
            if !name
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            {
                return None;
            }
            Some((offset, std::str::from_utf8(name).ok()?.to_owned()))
        })
        .collect::<Vec<_>>();
    declarations
        .iter()
        .enumerate()
        .map(|(index, (offset, name))| ClassInterval {
            name: name.clone(),
            class_offset: *offset,
            content_start: offset + 6 + name.len(),
            content_end: declarations
                .get(index + 1)
                .map_or(payload.len(), |(next, _)| *next),
        })
        .collect()
}

/// One validated face table. Positions are source `f32` metres.
#[derive(Debug, Clone, Default)]
pub(super) struct Table {
    pub(super) strips: Vec<u32>,
    pub(super) positions: Vec<[f32; 3]>,
    pub(super) normals: Vec<[f32; 3]>,
}

impl Table {
    /// Triangle corners from the strips, alternating winding per strip.
    pub(super) fn triangles(&self) -> Option<Vec<[u32; 3]>> {
        let mut triangles = Vec::new();
        let mut base = 0usize;
        for length in &self.strips {
            let length = usize::try_from(*length).ok()?;
            for i in 0..length.saturating_sub(2) {
                let corner = |offset: usize| u32::try_from(base + i + offset).ok();
                triangles.push(if i % 2 == 0 {
                    [corner(0)?, corner(1)?, corner(2)?]
                } else {
                    [corner(0)?, corner(2)?, corner(1)?]
                });
            }
            base = base.checked_add(length)?;
        }
        Some(triangles)
    }

    pub(super) fn triangle_count(&self) -> Option<usize> {
        self.strips.iter().try_fold(0usize, |total, length| {
            total.checked_add(usize::try_from(*length).ok()?.saturating_sub(2))
        })
    }
}

/// A face table with its byte ranges and saved persistent references.
#[derive(Debug, Clone)]
pub(super) struct Face {
    pub(super) table: Table,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) metadata_start: usize,
    pub(super) metadata_end: usize,
    pub(super) surface_references: Vec<SurfaceReference>,
}

impl Face {
    /// The feature source ID when every saved reference agrees.
    pub(super) fn feature_source_id(&self) -> Option<u32> {
        let mut sources = self
            .surface_references
            .iter()
            .map(|reference| reference.feature_source_id);
        let first = sources.next()?;
        sources.all(|source| source == first).then_some(first)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SurfaceReference {
    pub(super) identity: String,
    pub(super) feature_source_id: u32,
    pub(super) local_surface_id: u32,
}

struct Channel {
    item_size: u32,
    kind: u32,
    flags: u32,
    count: usize,
    data: std::ops::Range<usize>,
}

fn read_channel(bytes: &[u8], at: usize) -> Option<Channel> {
    let item_size = u32_le_at(bytes, at)?;
    let kind = u32_le_at(bytes, at + 4)?;
    let flags = u32_le_at(bytes, at + 8)?;
    let count = usize::try_from(u32_le_at(bytes, at + 12)?).ok()?;
    let start = at.checked_add(CHANNEL_HEADER_LEN)?;
    let end = start.checked_add(usize::try_from(item_size).ok()?.checked_mul(count)?)?;
    (end <= bytes.len()).then_some(Channel {
        item_size,
        kind,
        flags,
        count,
        data: start..end,
    })
}

fn read_vectors(bytes: &[u8], channel: &Channel) -> Option<Vec<[f32; 3]>> {
    (0..channel.count)
        .map(|index| {
            let at = channel.data.start + index * 12;
            let value = [
                f32_le_at(bytes, at)?,
                f32_le_at(bytes, at + 4)?,
                f32_le_at(bytes, at + 8)?,
            ];
            value.iter().all(|v| v.is_finite()).then_some(value)
        })
        .collect()
}

fn auxiliary_channels_agree(bytes: &[u8], strips: &[u32], channels: &[Channel]) -> bool {
    let [b, c, d] = channels else {
        return false;
    };
    if (b.item_size, b.kind, b.flags) != (4, 8, 2)
        || (c.item_size, c.kind, c.flags) != (4, 8, 2)
        || (d.item_size, d.kind, d.flags) != (1, 8, 2)
    {
        return false;
    }
    let Some(endpoint_lists) = strips
        .iter()
        .map(|length| length.checked_mul(2)?.checked_sub(2))
        .collect::<Option<Vec<u32>>>()
    else {
        return false;
    };
    let Some(endpoints) = endpoint_lists.iter().try_fold(0usize, |total, count| {
        total.checked_add(usize::try_from(*count).ok()?)
    }) else {
        return false;
    };
    let stored_lists = (0..c.count)
        .map(|index| u32_le_at(bytes, c.data.start + index * 4))
        .collect::<Option<Vec<u32>>>();
    let counts_agree = (b.count, d.count) == (0, 0) || (b.count, d.count) == (endpoints, endpoints);
    let floats_finite = (0..b.count)
        .all(|index| f32_le_at(bytes, b.data.start + index * 4).is_some_and(f32::is_finite));
    counts_agree
        && c.count == strips.len()
        && stored_lists.as_deref() == Some(endpoint_lists.as_slice())
        && floats_finite
}

/// Parse one six-channel table starting at `at`; return it and its end offset.
pub(super) fn parse_table(bytes: &[u8], at: usize) -> Option<(Table, usize)> {
    let mut channels = Vec::with_capacity(CHANNEL_COUNT);
    let mut cursor = at;
    for _ in 0..CHANNEL_COUNT {
        let channel = read_channel(bytes, cursor)?;
        cursor = channel.data.end;
        channels.push(channel);
    }
    let [strips, positions, normals, ..] = channels.as_slice() else {
        return None;
    };
    if (strips.item_size, strips.kind, strips.flags) != (4, 8, 2)
        || (positions.item_size, positions.kind, positions.flags) != (12, 100, 2)
        || (normals.item_size, normals.kind, normals.flags) != (12, 100, 2)
    {
        return None;
    }
    let strip_lengths = (0..strips.count)
        .map(|index| u32_le_at(bytes, strips.data.start + index * 4))
        .collect::<Option<Vec<u32>>>()?;
    let vertex_total = strip_lengths.iter().try_fold(0usize, |total, length| {
        total.checked_add(usize::try_from(*length).ok()?)
    })?;
    let table = Table {
        positions: read_vectors(bytes, positions)?,
        normals: read_vectors(bytes, normals)?,
        strips: strip_lengths,
    };
    if table.strips.is_empty()
        || table.positions.is_empty()
        || vertex_total != table.positions.len()
        || (!table.normals.is_empty() && table.normals.len() != table.positions.len())
        || !auxiliary_channels_agree(bytes, &table.strips, &channels[3..])
    {
        return None;
    }
    Some((table, cursor))
}

/// Size of the face header that precedes the first descriptor.
fn header_len(payload: &[u8], header: usize) -> usize {
    let extended = u32_le_at(payload, header + EXTENDED_FORM) == Some(1)
        && u32_le_at(payload, header + EXTENDED_ZERO_AT_12) == Some(0)
        && u32_le_at(payload, header + EXTENDED_ZERO_AT_16) == Some(0)
        && u32_le_at(payload, header + EXTENDED_FORM_TOKEN).is_some_and(|token| token != 0)
        && payload
            .get(header + EXTENDED_ZERO_TAIL..header + EXTENDED_HEADER_LEN)
            .is_some_and(|tail| tail.iter().all(|byte| *byte == 0));
    if extended {
        EXTENDED_HEADER_LEN
    } else {
        COMPACT_HEADER_LEN
    }
}

fn header_agrees(payload: &[u8], header: usize, table: &Table) -> bool {
    let counts = (
        u32_le_at(payload, header + HEADER_TRIANGLE_COUNT).and_then(|v| usize::try_from(v).ok()),
        u32_le_at(payload, header + HEADER_STRIP_COUNT).and_then(|v| usize::try_from(v).ok()),
    );
    counts == (table.triangle_count(), Some(table.strips.len()))
}

/// The first table and any later tables found before `limit`.
fn parse_table_sequence(
    payload: &[u8],
    at: usize,
    limit: usize,
) -> Option<Vec<(usize, usize, Table)>> {
    let (first, mut cursor) = parse_table(payload, at)?;
    if cursor > limit {
        return None;
    }
    let mut tables = vec![(at, cursor, first)];
    while cursor + CHANNEL_HEADER_LEN <= limit {
        let Some(relative) = payload
            .get(cursor..limit)
            .and_then(|window| find_first(window, &[4, 0, 0, 0]))
        else {
            break;
        };
        cursor += relative;
        match parse_table(payload, cursor) {
            Some((next, end)) if end <= limit => {
                tables.push((cursor, end, next));
                cursor = end;
            }
            _ => cursor += 4,
        }
    }
    Some(tables)
}

/// Every validated face table in one decoded `DisplayLists` payload, in byte order.
pub(super) fn faces(payload: &[u8], classes: &[ClassInterval]) -> Vec<Face> {
    let markers = find_all(payload, FACE_TESSELLATION_CLASS);
    let mut faces = Vec::new();
    for (index, marker) in markers.iter().copied().enumerate() {
        let header = marker + FACE_TESSELLATION_CLASS.len();
        let (Some(triangle_count), Some(strip_count)) = (
            u32_le_at(payload, header + HEADER_TRIANGLE_COUNT),
            u32_le_at(payload, header + HEADER_STRIP_COUNT),
        ) else {
            continue;
        };
        let limit = classes
            .iter()
            .find(|class| {
                class.name.as_bytes() == FACE_TESSELLATION_CLASS && class.class_offset + 6 == marker
            })
            .map_or_else(
                || markers.get(index + 1).copied().unwrap_or(payload.len()),
                |class| class.content_end,
            );
        let start = header + header_len(payload, header);
        let Some(tables) = parse_table_sequence(payload, start, limit) else {
            continue;
        };
        let agrees = tables.first().is_some_and(|(_, _, table)| {
            usize::try_from(triangle_count).ok() == table.triangle_count()
                && usize::try_from(strip_count).ok() == Some(table.strips.len())
        });
        if !agrees {
            continue;
        }
        for (start, end, table) in tables {
            faces.push(Face {
                table,
                start,
                end,
                metadata_start: end,
                metadata_end: limit,
                surface_references: Vec::new(),
            });
        }
    }
    append_referenced_faces(payload, classes, &mut faces);
    faces.sort_by_key(|face| face.start);
    for index in 0..faces.len() {
        let next_start = faces.get(index + 1).map(|next| next.start);
        let face = &mut faces[index];
        if let Some(next_start) = next_start {
            face.metadata_end = face.metadata_end.min(next_start);
        }
        face.surface_references =
            surface_references(payload, face.metadata_start, face.metadata_end);
    }
    faces
}

/// Recognize tables that reuse the face class through one MFC old-class tag.
///
/// The tag must be shared by every validated instance in the declared face
/// interval. Unknown tags and bare descriptor arrays are not admitted.
fn append_referenced_faces(payload: &[u8], classes: &[ClassInterval], faces: &mut Vec<Face>) {
    let mut tags = HashSet::new();
    for face in faces.iter() {
        for width in [COMPACT_HEADER_LEN, EXTENDED_HEADER_LEN] {
            let Some(at) = face.start.checked_sub(width + 2) else {
                continue;
            };
            let Some(tag) = u16_le_at(payload, at) else {
                continue;
            };
            if tag & 0x8000 == 0 || tag == 0xffff || tag == 0x8000 {
                continue;
            }
            let inside_face_class = classes.iter().any(|class| {
                class.name.as_bytes() == FACE_TESSELLATION_CLASS
                    && class.content_start <= at
                    && face.end <= class.content_end
            });
            if inside_face_class
                && header_len(payload, at + 2) == width
                && header_agrees(payload, at + 2, &face.table)
            {
                tags.insert(tag);
            }
        }
    }
    let tags = tags.into_iter().collect::<Vec<u16>>();
    let [tag] = tags.as_slice() else {
        return;
    };
    let tag = *tag;
    let Some(first) = faces.iter().map(|face| face.start).min() else {
        return;
    };
    let known = faces
        .iter()
        .map(|face| (face.start, face.end))
        .collect::<HashMap<_, _>>();
    let mut ranges = faces
        .iter()
        .map(|face| (face.start, face.end))
        .collect::<Vec<_>>();
    ranges.sort_unstable();
    let mut ranges = ranges.into_iter().peekable();
    let tag_bytes = tag.to_le_bytes();
    let mut at = first;
    while at + 2 < payload.len() {
        if let Some(&(start, end)) = ranges.peek()
            && start <= at
        {
            ranges.next();
            at = at.max(end);
            continue;
        }
        if payload.get(at..at + 2) != Some(tag_bytes.as_slice()) {
            at += 1;
            continue;
        }
        let header = at + 2;
        let start = header + header_len(payload, header);
        if let Some(end) = known.get(&start) {
            at = *end;
            continue;
        }
        let limit = classes
            .iter()
            .find(|class| class.class_offset > at)
            .map_or(payload.len(), |class| class.class_offset);
        if let Some((table, end)) = parse_table(payload, start)
            && end <= limit
            && header_agrees(payload, header, &table)
        {
            faces.push(Face {
                table,
                start,
                end,
                metadata_start: end,
                metadata_end: limit,
                surface_references: Vec::new(),
            });
            at = end;
            continue;
        }
        at += 2;
    }
}

/// Bounded UTF-16LE strings framed by `FF FE FF <u8 count>`.
fn utf16_strings(payload: &[u8], start: usize, end: usize) -> Vec<(usize, usize, String)> {
    let mut strings = Vec::new();
    let mut at = start;
    let end = end.min(payload.len());
    while at + 4 <= end {
        if payload.get(at..at + 3) != Some(UTF16_MARKER.as_slice()) {
            at += 1;
            continue;
        }
        let count = usize::from(payload[at + 3]);
        let text_end = at + 4 + count * 2;
        let Some(raw) = payload.get(at + 4..text_end).filter(|_| text_end <= end) else {
            at += 1;
            continue;
        };
        let units = raw
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        if let Ok(text) = String::from_utf16(&units) {
            strings.push((at, text_end, text));
        }
        at = text_end;
    }
    strings
}

/// `mo*SurfIdRep_c,<feature>,<local>,...` references in a face's metadata slot.
fn surface_references(payload: &[u8], start: usize, end: usize) -> Vec<SurfaceReference> {
    utf16_strings(payload, start, end)
        .into_iter()
        .filter_map(|(_, _, text)| {
            let identity = text.trim().to_owned();
            let mut fields = identity.split(',');
            fields
                .next()
                .filter(|name| name.starts_with("mo") && name.ends_with("SurfIdRep_c"))?;
            let feature_source_id = fields
                .next()?
                .parse::<u32>()
                .ok()
                .filter(|source| *source != 0 && *source != u32::MAX)?;
            let local_surface_id = fields.next()?.parse::<u32>().ok()?;
            Some(SurfaceReference {
                identity: identity.clone(),
                feature_source_id,
                local_surface_id,
            })
        })
        .collect()
}

/// Non-empty bounded UTF-16 names after the final table. Candidates only.
pub(super) fn trailing_names(payload: &[u8], start: usize) -> Vec<String> {
    let mut names = utf16_strings(payload, start, payload.len())
        .into_iter()
        .map(|(_, _, text)| text)
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}
