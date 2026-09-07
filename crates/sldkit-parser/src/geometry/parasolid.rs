//! Parasolid header validation and the existing sldkit byte-domain convention.

use parasolid_core::{InspectionLimits, inspect_xb};
use sldkit_core::ResourceLimits;

/// The legacy geometry domain begins immediately after the schema string,
/// before max-type/user-field framing. Keep that public coordinate system.
pub(super) fn header(payload: &[u8], limits: &ResourceLimits) -> Option<(String, String, usize)> {
    if !payload.starts_with(b"PS\0\0") {
        return None;
    }
    let inspected = inspect_xb(
        payload,
        InspectionLimits {
            max_file_size: usize::try_from(limits.max_file_size).unwrap_or(usize::MAX),
            max_string_bytes: usize::try_from(limits.max_string_bytes).unwrap_or(usize::MAX),
        },
    );
    if let Ok(header) = inspected {
        let trailer_size = 4 + usize::from(header.schema_max_type.is_some()) * 2;
        let body_offset = header.binary_header_range.end.checked_sub(trailer_size)?;
        return Some((header.modeller_version, header.schema_key, body_offset));
    }
    writer_header(payload, limits)
}

/// cadmpeg 0.5.3's source-less writer uses a three-byte schema length and omits
/// the standard max-type/user-field trailer. This is a separate, exact writer
/// profile; a malformed numeric Parasolid key cannot enter this branch.
fn writer_header(payload: &[u8], limits: &ResourceLimits) -> Option<(String, String, usize)> {
    if u64::try_from(payload.len()).ok()? > limits.max_file_size {
        return None;
    }
    let length = usize::from(u16::from_be_bytes([*payload.get(4)?, *payload.get(5)?]));
    if u64::try_from(length).ok()? > limits.max_string_bytes {
        return None;
    }
    let end = 6_usize.checked_add(length)?;
    let description = std::str::from_utf8(payload.get(6..end)?).ok()?;
    if !matches!(description, "partition body" | "deltas body") {
        return None;
    }
    let prefix = payload.get(end..end.checked_add(3)?)?;
    if prefix[..2] != [0, 0] {
        return None;
    }
    let start = end.checked_add(3)?;
    let schema_end = start.checked_add(usize::from(prefix[2]))?;
    let schema = std::str::from_utf8(payload.get(start..schema_end)?).ok()?;
    if !matches!(schema, "SCH_SW_32001_11000" | "SCH_SW_33103_11000")
        || u64::try_from(schema.len()).ok()? > limits.max_string_bytes
    {
        return None;
    }
    Some((description.to_owned(), schema.to_owned(), schema_end))
}

#[cfg(test)]
mod tests {
    use super::header;
    use sldkit_core::ResourceLimits;

    fn native(key: &str, embedded: bool) -> (Vec<u8>, usize) {
        let mut data = b"PS\0\0\0\x09partition".to_vec();
        data.extend_from_slice(&i32::try_from(key.len()).unwrap_or(0).to_be_bytes());
        data.extend_from_slice(key.as_bytes());
        let schema_end = data.len();
        if embedded {
            data.extend_from_slice(&239_u16.to_be_bytes());
        }
        data.extend_from_slice(&0_i32.to_be_bytes());
        (data, schema_end)
    }

    #[test]
    fn native_header_keeps_domain_origin_for_both_key_forms() {
        for (key, embedded) in [
            ("SCH_3701229_37102_13006", true),
            ("SCH_1300000_13006", false),
        ] {
            let (data, schema_end) = native(key, embedded);
            let actual = header(&data, &ResourceLimits::service());
            assert_eq!(actual, Some(("partition".into(), key.into(), schema_end)));
            for end in 0..data.len() {
                assert!(header(&data[..end], &ResourceLimits::service()).is_none());
            }
        }
    }

    #[test]
    fn malformed_numeric_headers_do_not_use_writer_compatibility() {
        let (mut data, end) = native("SCH_3701229_37102_13006", true);
        data[end + 2..].copy_from_slice(&17_i32.to_be_bytes());
        assert!(header(&data, &ResourceLimits::service()).is_none());
        let (mut data, _) = native("SCH_3701229_37102_13006", true);
        data[15..19].copy_from_slice(&999_i32.to_be_bytes());
        assert!(header(&data, &ResourceLimits::service()).is_none());
    }

    #[test]
    fn native_header_respects_caller_limits() {
        let (data, _) = native("SCH_3701229_37102_13006", true);
        let mut limits = ResourceLimits::service();
        limits.max_string_bytes = 8;
        assert!(header(&data, &limits).is_none());
        limits = ResourceLimits::service();
        limits.max_file_size = 8;
        assert!(header(&data, &limits).is_none());
    }

    #[test]
    fn only_exact_source_less_writer_profiles_are_admitted() {
        for key in [
            "SCH_SW_32001_11000",
            "SCH_SW_33103_11000",
            "SCH_SW_99999_11000",
        ] {
            let mut data = b"PS\0\0\0\x0epartition body\0\0".to_vec();
            data.push(u8::try_from(key.len()).unwrap_or(0));
            data.extend_from_slice(key.as_bytes());
            assert_eq!(
                header(&data, &ResourceLimits::service()).is_some(),
                key != "SCH_SW_99999_11000",
            );
        }
    }
}
