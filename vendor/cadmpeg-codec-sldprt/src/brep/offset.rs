//! Unit conversion for the shared partial offset-surface reader.
pub(crate) use parasolid_core::partial::offset::OffsetCarrier;
use std::collections::HashMap;
pub(crate) fn scan(bytes: &[u8]) -> HashMap<u16, OffsetCarrier> {
    let mut result = parasolid_core::partial::offset::scan(bytes);
    result.retain(|_, value| {
        value.distance *= super::LEN_TO_MM;
        value.distance.is_finite()
    });
    result
}
