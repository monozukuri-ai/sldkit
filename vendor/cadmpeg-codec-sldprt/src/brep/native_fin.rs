// SPDX-License-Identifier: Apache-2.0
// Modified by sldkit; see PATCHES.md.
//! Adapt verified vertexless circle rings to the graph's derived seam profile.
//!
//! Ordinary FIN interpretation stays in parasolid-core. A native ring edge has
//! null vertices at both ends. The IR adapter already represents a circle ring
//! with an explicitly derived seam vertex; no native vertex is invented here.
//! This adapter is called only after the exact native hierarchy/schema gate.

use super::{topology::Tables, CarrierGeometry, CarrierIndex};
use cadmpeg_ir::geometry::CurveGeometry;

pub(super) fn normalize_or_withhold(tables: &mut Tables, carriers: &CarrierIndex) -> usize {
    let count = tables.coedges.len();
    let rings: Vec<_> = tables
        .coedges
        .values()
        .filter(|fin| fin.refs.get(4).is_some_and(|vertex| *vertex <= 1))
        .map(|fin| fin.attr)
        .collect();
    if rings.is_empty() {
        return parasolid_core::partial::native_fin::normalize_or_withhold(tables);
    }
    if !rings
        .iter()
        .all(|id| valid_circle_fin(*id, tables, carriers))
    {
        tables.coedges.clear();
        return count;
    }

    // Keep the core's complete validation for every ordinary FIN, without
    // inserting fake vertices or relaxing missing-reference checks. A ring
    // pair is disconnected from all ordinary FIN vertex/loop chains.
    let mut closed = Vec::with_capacity(rings.len());
    for id in rings {
        if let Some(fin) = tables.coedges.remove(&id) {
            closed.push(fin);
        }
    }
    if !tables.coedges.is_empty()
        && parasolid_core::partial::native_fin::normalize_or_withhold(tables) != 0
    {
        return count;
    }
    for mut fin in closed {
        fin.refs.swap(2, 3);
        // Both directed endpoints remain the native null sentinel.
        tables.coedges.insert(fin.attr, fin);
    }
    0
}

fn valid_circle_fin(id: u16, tables: &Tables, carriers: &CarrierIndex) -> bool {
    let fin = &tables.coedges[&id];
    if fin.refs.len() != 9
        || fin.refs[4] != 1
        || fin.refs[7] != 1
        || fin.refs[8] != 1
        || !matches!(fin.marker, Some(b'+' | b'-'))
    {
        return false;
    }
    let Some(other) = tables.coedges.get(&fin.refs[5]) else {
        return false;
    };
    if other.attr == id
        || other.refs.len() != 9
        || other.refs[4] != 1
        || other.refs[5] != id
        || other.refs[6] != fin.refs[6]
        || other.marker == fin.marker
        || (fin.refs[1] <= 1 && other.refs[1] <= 1)
    {
        return false;
    }
    if fin.refs[1] == 1 {
        if fin.refs[2] != 1 || fin.refs[3] != 1 {
            return false;
        }
    } else {
        let Some(loop_) = tables.loops.get(&fin.refs[1]) else {
            return false;
        };
        if fin.refs[2] != id
            || fin.refs[3] != id
            || loop_.refs.get(1) != Some(&id)
            || !loop_
                .refs
                .get(2)
                .is_some_and(|face| tables.bridges.contains_key(face))
        {
            return false;
        }
    }
    let Some(edge) = tables.edge_uses.get(&fin.refs[6]) else {
        return false;
    };
    let Some(curve) = edge.refs.get(3).and_then(|attr| carriers.curve(*attr)) else {
        return false;
    };
    // A bounded curve wrapper or an untyped/derived curve cannot prove a full
    // circle. Elliptic/NURBS ring support needs its own seam mapping contract.
    curve.parameter_range.is_none()
        && !carriers.curve_is_derived(curve.attr)
        && matches!(&curve.geometry, CarrierGeometry::Curve(CurveGeometry::Circle { radius, .. }) if radius.is_finite() && *radius > 0.0)
}

#[cfg(test)]
mod tests;
