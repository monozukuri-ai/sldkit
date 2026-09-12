// SPDX-License-Identifier: Apache-2.0
// Modified by sldkit; see the crate-root PATCHES.md.
//! Conservative joins from display persistent identities to emitted topology.

use std::collections::{HashMap, HashSet};

use cadmpeg_ir::document::Model;

use super::PersistentSurfaceReference;

/// Source references stay scoped to their display stream. A configuration name
/// is only corroboration; it cannot choose between ambiguous topology matches.
#[derive(Default)]
pub(crate) struct DisplayOwnership {
    pub(crate) references: HashMap<String, (usize, Vec<PersistentSurfaceReference>)>,
    pub(crate) configuration_names: HashMap<usize, HashSet<String>>,
}

/// Read bounded MFC UTF-16 strings after the final descriptor table. These are
/// candidates only: face identities and complete configuration membership must
/// independently select the same single configuration before narrowing owners.
pub(crate) fn trailing_names(payload: &[u8], start: usize) -> HashSet<String> {
    let mut names = HashSet::new();
    let mut at = start;
    while at + 4 <= payload.len() {
        if payload.get(at..at + 3) != Some(&[0xff, 0xfe, 0xff]) {
            at += 1;
            continue;
        }
        let end = at + 4 + usize::from(payload[at + 3]) * 2;
        let Some(raw) = payload.get(at + 4..end) else {
            break;
        };
        let units = raw
            .chunks_exact(2)
            .map(|p| u16::from_le_bytes([p[0], p[1]]))
            .collect::<Vec<_>>();
        if let Ok(name) = String::from_utf16(&units) {
            if !name.is_empty() {
                names.insert(name);
            }
        }
        at = end;
    }
    names
}

pub(super) fn face_constraints(
    model: &Model,
    source: &DisplayOwnership,
    identities: &[(String, u32, u32)],
) -> HashMap<String, HashSet<String>> {
    let regions = model
        .regions
        .iter()
        .map(|r| (&r.id, &r.body))
        .collect::<HashMap<_, _>>();
    let shells = model
        .shells
        .iter()
        .filter_map(|s| Some((&s.id, *regions.get(&s.region)?)))
        .collect::<HashMap<_, _>>();
    let face_bodies = model
        .faces
        .iter()
        .filter_map(|f| Some((f.id.0.as_str(), *shells.get(&f.shell)?)))
        .collect::<HashMap<_, _>>();
    let mut index = HashMap::<(u32, u32), HashSet<String>>::new();
    for (face, feature, local) in identities {
        if face_bodies.contains_key(face.as_str()) {
            index
                .entry((*feature, *local))
                .or_default()
                .insert(face.clone());
        }
    }
    let mut constraints = HashMap::new();
    let mut sections = HashMap::<usize, Vec<&str>>::new();
    for mesh in &model.tessellations {
        let Some((section, refs)) = source.references.get(&mesh.id) else {
            continue;
        };
        sections.entry(*section).or_default().push(&mesh.id);
        let Some(first) = refs.first() else { continue };
        let candidates = if refs.iter().all(|r| r == first) {
            index
                .get(&(first.feature_source_id, first.local_surface_id))
                .cloned()
                .unwrap_or_default()
        } else {
            HashSet::new()
        };
        // An empty constraint rejects conflicting/unmatched source references;
        // it must not fall back to a coincident analytic surface.
        constraints.insert(mesh.id.clone(), candidates);
    }
    if model.configurations.is_empty()
        || model
            .configurations
            .iter()
            .any(|c| c.bodies.is_unresolved())
    {
        return constraints;
    }
    for (section, meshes) in sections {
        if meshes
            .iter()
            .any(|id| constraints.get(*id).is_none_or(HashSet::is_empty))
        {
            continue;
        }
        let matching = model
            .configurations
            .iter()
            .filter(|configuration| {
                meshes.iter().all(|id| {
                    constraints[*id].iter().any(|face| {
                        face_bodies
                            .get(face.as_str())
                            .is_some_and(|body| configuration.bodies.iter().any(|b| b == *body))
                    })
                })
            })
            .collect::<Vec<_>>();
        let [configuration] = matching.as_slice() else {
            continue;
        };
        let Some(name) = configuration.name.resolved() else {
            continue;
        };
        let Some(names) = source.configuration_names.get(&section) else {
            continue;
        };
        let named = model
            .configurations
            .iter()
            .filter(|c| c.name.resolved().is_some_and(|n| names.contains(n)))
            .count();
        if named != 1 || !names.contains(name) {
            continue;
        }
        for id in meshes {
            if let Some(faces) = constraints.get_mut(id) {
                faces.retain(|face| {
                    face_bodies
                        .get(face.as_str())
                        .is_some_and(|body| configuration.bodies.iter().any(|b| b == *body))
                });
            }
        }
    }
    constraints
}

#[cfg(test)]
mod tests;
