//! Convert native ownership values from parasolid-core into the existing IR adapter.
use super::{
    entity::{BodyRecord, RegionRecord, ShellRecord},
    topology::Tables,
};
use cadmpeg_ir::topology::BodyKind;

pub(super) fn recover(streams: &[(&[u8], &str, bool)], tables: &Tables) -> Option<Vec<BodyRecord>> {
    parasolid_core::partial::native_hierarchy::recover(streams, tables).map(|bodies| {
        bodies
            .into_iter()
            .map(|body| BodyRecord {
                attr: body.attr,
                kind: match body.kind {
                    parasolid_core::brep::BodyKind::Sheet => BodyKind::Sheet,
                    parasolid_core::brep::BodyKind::Solid => BodyKind::Solid,
                    parasolid_core::brep::BodyKind::Wire => BodyKind::Wire,
                    parasolid_core::brep::BodyKind::General => BodyKind::General,
                },
                refs: body.refs,
                offset: body.offset,
                regions: body
                    .regions
                    .into_iter()
                    .map(|region| RegionRecord {
                        attr: region.attr,
                        offset: region.offset,
                        shells: region
                            .shells
                            .into_iter()
                            .map(|shell| ShellRecord {
                                attr: shell.attr,
                                offset: shell.offset,
                                refs: shell.refs,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect()
    })
}
pub(super) fn verified_read_spans(
    data: &[u8],
    schema: &str,
    tables: &Tables,
) -> Vec<crate::byte_ledger::DecodedSpan> {
    parasolid_core::partial::native_hierarchy::verified_read_spans(data, schema, tables)
        .into_iter()
        .map(Into::into)
        .collect()
}

#[cfg(test)]
mod tests;
