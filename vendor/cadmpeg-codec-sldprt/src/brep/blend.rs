//! Unit conversion for shared rolling-ball and support-pair readers.
pub(crate) use parasolid_core::partial::blend::{
    BlendCarrier, BlendSupportRef, SupportPairCarrier,
};
use std::collections::HashMap;
pub(crate) fn scan(bytes: &[u8]) -> (HashMap<u16, BlendCarrier>, HashMap<u16, SupportPairCarrier>) {
    let (mut blends, pairs) = parasolid_core::partial::blend::scan(bytes);
    for blend in blends.values_mut() {
        blend.signed_radius *= super::LEN_TO_MM;
    }
    (blends, pairs)
}
