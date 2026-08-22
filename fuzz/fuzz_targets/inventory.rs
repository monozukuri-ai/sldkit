#![no_main]

use std::io::Write;

use flate2::{Compression, write::DeflateEncoder};
use libfuzzer_sys::fuzz_target;
use sldkit_core::ResourceLimits;

const MODERN_MARKER: &[u8] = b"\x14\x00\x06\x00\x08\x00";
const MAX_XML_FUZZ_BYTES: usize = 64 * 1024;

fn modern_xml_candidate(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() > MAX_XML_FUZZ_BYTES {
        return None;
    }
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data).ok()?;
    let compressed = encoder.finish().ok()?;
    let name = b"docProps/custom.xml";
    let encoded_name = name
        .iter()
        .map(|value| value.rotate_right(4))
        .collect::<Vec<_>>();
    let compressed_len = u32::try_from(compressed.len()).ok()?;
    let decoded_len = u32::try_from(data.len()).ok()?;
    let name_len = u32::try_from(encoded_name.len()).ok()?;

    let mut output = b"SLDK\x00\x00\x00\x04".to_vec();
    output.extend_from_slice(MODERN_MARKER);
    output.extend_from_slice(&7_u32.to_le_bytes());
    output.extend_from_slice(&crc32fast::hash(data).to_le_bytes());
    output.extend_from_slice(&compressed_len.to_le_bytes());
    output.extend_from_slice(&decoded_len.to_le_bytes());
    output.extend_from_slice(&name_len.to_le_bytes());
    output.extend_from_slice(&encoded_name);
    output.extend_from_slice(&compressed);
    Some(output)
}

fuzz_target!(|data: &[u8]| {
    let mut limits = ResourceLimits::service();
    limits.max_file_size = 4 * 1024 * 1024;
    limits.max_total_uncompressed_bytes = 8 * 1024 * 1024;
    limits.max_stream_count = 4096;
    limits.max_string_bytes = 64 * 1024;
    limits.max_xml_stream_bytes = 256 * 1024;
    limits.max_xml_nodes = 4096;
    limits.max_nesting_depth = 32;

    let _ = sldkit_parser::probe_bytes(data, &limits);
    let _ = sldkit_parser::inspect_bytes(data, None, &limits);
    let _ = sldkit_parser::parse_bytes(data, None, &limits);
    if let Some(candidate) = modern_xml_candidate(data) {
        let _ = sldkit_parser::parse_bytes(&candidate, Some("fuzz.SLDPRT"), &limits);
    }
});
