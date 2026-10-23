// SPDX-License-Identifier: Apache-2.0
// Adopted from cadmpeg-codec-sldprt 0.5.3 `src/appearance.rs` (Apache-2.0)
// and rewritten for sldkit-owned types; see THIRD_PARTY_NOTICES.md.
//! Saved appearance colors for display faces.
//!
//! Precedence is body (`uoBodyPropInfo_c`), then feature
//! (`ThirdPtyStore/VisualStates`), then face-local inline properties.

use std::collections::HashMap;

use sldkit_core::DisplayColorSource;

use super::tables::{ClassInterval, FACE_TESSELLATION_CLASS, Face, find_all, u32_le_at};

const UTF16_MARKER: [u8; 3] = [0xff, 0xfe, 0xff];
const BODY_PROPERTIES_CLASS: &str = "uoBodyPropInfo_c";
const COMPONENT_FEATURE_CLASS: &str = "moCompFeature_c";

// Inline visual-properties prefix: marker, packed color, 12 uninterpreted
// bytes, then the framed UTF-16 appearance name.
const INLINE_MARKER: [u8; 2] = [0x33, 0x80];
const INLINE_PACKED_COLOR: usize = 2;
const NAME_AFTER_PACKED_COLOR: usize = 16;

// VisualStates feature-appearance record (36 bytes), located by its marker.
const FEATURE_RECORD_LEN: usize = 36;
const FEATURE_MARKER: [u8; 2] = [0x09, 0x80];
const FEATURE_MARKER_OFFSET: usize = 30;
const FEATURE_VERSION: (usize, u32) = (0, 0x0000_4268);
const FEATURE_SOURCE_ID: usize = 4;
const FEATURE_TIMESTAMP: usize = 8;
const FEATURE_SELECTORS: [(usize, u32); 3] = [(12, 1), (16, 1), (20, 2)];
const FEATURE_INSTANCE_PREFIX: (usize, [u8; 6]) = (24, [0x07, 0x80, 0x01, 0x00, 0x00, 0x00]);
const FEATURE_PACKED_COLOR: usize = 32;

/// The effective saved appearance of one display face.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Appearance {
    pub(super) name: Option<String>,
    pub(super) color: [f32; 4],
    pub(super) source: DisplayColorSource,
}

/// Packed `0x00BBGGRR` to opaque RGBA in `[0, 1]`.
fn packed_rgba(packed: u32) -> [f32; 4] {
    let [r, g, b, _] = packed.to_le_bytes();
    [
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    ]
}

/// A named definition whose packed color starts at `packed_offset`.
fn definition_at(payload: &[u8], packed_offset: usize) -> Option<(String, u32)> {
    let packed = u32_le_at(payload, packed_offset)?;
    let header = packed_offset.checked_add(NAME_AFTER_PACKED_COLOR)?;
    if payload.get(header..header + 3) != Some(UTF16_MARKER.as_slice()) {
        return None;
    }
    let count = usize::from(*payload.get(header + 3)?);
    let start = header + 4;
    let raw = payload.get(start..start.checked_add(count.checked_mul(2)?)?)?;
    let units = raw
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let name = String::from_utf16(&units).ok()?.trim().to_owned();
    (!name.is_empty()).then_some((name, packed))
}

fn inline_definitions(payload: &[u8], start: usize, end: usize) -> Vec<(String, u32)> {
    let Some(window) = payload.get(start..end.min(payload.len())) else {
        return Vec::new();
    };
    find_all(window, &INLINE_MARKER)
        .into_iter()
        .filter_map(|relative| definition_at(payload, start + relative + INLINE_PACKED_COLOR))
        .collect()
}

/// Feature-source colors from `ThirdPtyStore/VisualStates`; conflicting duplicates are dropped.
pub(super) fn feature_colors(visual_states: &[u8]) -> HashMap<u32, u32> {
    let classes = super::tables::class_intervals(visual_states);
    let mut candidates = HashMap::<u32, Option<(u32, u32)>>::new();
    for marker in find_all(visual_states, &FEATURE_MARKER) {
        let Some(at) = marker.checked_sub(FEATURE_MARKER_OFFSET) else {
            continue;
        };
        let Some(record) = visual_states.get(at..at + FEATURE_RECORD_LEN) else {
            continue;
        };
        let in_feature_class = classes.iter().any(|class| {
            class.name == COMPONENT_FEATURE_CLASS
                && class.content_start <= at
                && at + FEATURE_RECORD_LEN <= class.content_end
        });
        let selectors_agree = FEATURE_SELECTORS
            .iter()
            .all(|(offset, value)| u32_le_at(record, *offset) == Some(*value));
        let (prefix_at, prefix) = FEATURE_INSTANCE_PREFIX;
        if !in_feature_class
            || u32_le_at(record, FEATURE_VERSION.0) != Some(FEATURE_VERSION.1)
            || !selectors_agree
            || record.get(prefix_at..prefix_at + prefix.len()) != Some(prefix.as_slice())
        {
            continue;
        }
        let valid = |value: &u32| *value != 0 && *value != u32::MAX;
        let (Some(source), Some(timestamp), Some(packed)) = (
            u32_le_at(record, FEATURE_SOURCE_ID).filter(valid),
            u32_le_at(record, FEATURE_TIMESTAMP).filter(valid),
            u32_le_at(record, FEATURE_PACKED_COLOR),
        ) else {
            continue;
        };
        candidates
            .entry(source)
            .and_modify(|existing| {
                if existing.is_some_and(|previous| previous != (timestamp, packed)) {
                    *existing = None;
                }
            })
            .or_insert(Some((timestamp, packed)));
    }
    candidates
        .into_iter()
        .filter_map(|(source, value)| value.map(|(_, packed)| (source, packed)))
        .collect()
}

/// Resolve each face's appearance with body, feature, then face precedence.
pub(super) fn resolve(
    payload: &[u8],
    classes: &[ClassInterval],
    faces: &[Face],
    feature_colors: &HashMap<u32, u32>,
) -> Vec<Option<Appearance>> {
    let mut resolved = vec![None; faces.len()];
    let mut previous_body_end = 0usize;
    for class in classes {
        if class.name != BODY_PROPERTIES_CLASS {
            continue;
        }
        let definitions = inline_definitions(payload, class.content_start, class.content_end);
        if let [(name, packed)] = definitions.as_slice() {
            for (index, face) in faces.iter().enumerate() {
                if previous_body_end <= face.start && face.end <= class.class_offset {
                    resolved[index] = Some(Appearance {
                        name: Some(name.clone()),
                        color: packed_rgba(*packed),
                        source: DisplayColorSource::Body,
                    });
                }
            }
        }
        previous_body_end = class.content_end;
    }
    for (index, face) in faces.iter().enumerate() {
        if let Some(packed) = face
            .feature_source_id()
            .and_then(|source| feature_colors.get(&source))
        {
            resolved[index] = Some(Appearance {
                name: None,
                color: packed_rgba(*packed),
                source: DisplayColorSource::Feature,
            });
        }
    }
    for (index, face) in faces.iter().enumerate() {
        let Some(class) = classes.iter().find(|class| {
            class.name.as_bytes() == FACE_TESSELLATION_CLASS
                && class.content_start <= face.start
                && face.start < class.content_end
        }) else {
            continue;
        };
        let definitions = inline_definitions(
            payload,
            face.metadata_start,
            face.metadata_end.min(class.content_end),
        );
        if let [(name, packed)] = definitions.as_slice() {
            resolved[index] = Some(Appearance {
                name: Some(name.clone()),
                color: packed_rgba(*packed),
                source: DisplayColorSource::Face,
            });
        }
    }
    resolved
}
