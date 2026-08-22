use std::collections::{BTreeMap, BTreeSet};

use crc32fast::hash as crc32;
use sldkit_core::{
    ByteRange, ChecksumStatus, CompressionMethod, ContainerInventory, CoverageReport, Diagnostic,
    DiagnosticKind, DiagnosticSeverity, Envelope, InventoryEntry, InventoryEntryKind,
    InventoryEntryState, InventoryResult, InventoryStatus, ResourceLimits,
};

use crate::common::{
    RangeSet, ScanArtifacts, checked_end, inflate_diagnostic, inflate_raw_bounded, range_sum,
    saturating_u64, sha256_hex, u32_be, u32_le, usize_from_u64,
};

pub(crate) const MARKER: &[u8; 6] = b"\x14\x00\x06\x00\x08\x00";
const OUTER_HEADER_LEN: usize = 8;
const FRAME_HEADER_LEN: usize = 26;
const DIRECTORY_PREFIX_LEN: usize = 40;
const DIRECTORY_TRAILER_LEN: usize = 6;

struct ModernScanner<'a> {
    data: &'a [u8],
    limits: &'a ResourceLimits,
    entries: Vec<InventoryEntry>,
    diagnostics: Vec<Diagnostic>,
    stored: BTreeMap<String, Vec<u8>>,
    decoded: BTreeMap<String, Vec<u8>>,
    accounted: RangeSet,
    decoded_bytes: u64,
    decoded_streams: u64,
    marker_count: u64,
    fatal: bool,
    wanted_entry: Option<&'a str>,
    wanted_entries: Option<&'a BTreeSet<String>>,
}

pub(crate) fn scan(
    data: &[u8],
    limits: &ResourceLimits,
    wanted_entry: Option<&str>,
) -> ScanArtifacts {
    scan_impl(data, limits, wanted_entry, None)
}

pub(crate) fn scan_selected(
    data: &[u8],
    limits: &ResourceLimits,
    wanted_entries: &BTreeSet<String>,
) -> ScanArtifacts {
    scan_impl(data, limits, None, Some(wanted_entries))
}

fn scan_impl<'a>(
    data: &'a [u8],
    limits: &'a ResourceLimits,
    wanted_entry: Option<&'a str>,
    wanted_entries: Option<&'a BTreeSet<String>>,
) -> ScanArtifacts {
    let scanner = ModernScanner {
        data,
        limits,
        entries: Vec::new(),
        diagnostics: Vec::new(),
        stored: BTreeMap::new(),
        decoded: BTreeMap::new(),
        accounted: RangeSet::default(),
        decoded_bytes: 0,
        decoded_streams: 0,
        marker_count: 0,
        fatal: false,
        wanted_entry,
        wanted_entries,
    };
    scanner.walk()
}

impl ModernScanner<'_> {
    fn walk(mut self) -> ScanArtifacts {
        if self.data.len() < OUTER_HEADER_LEN {
            self.diagnostics.push(
                Diagnostic::new(
                    "modern.truncated_outer_header",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "modern container ends before its eight-byte outer header",
                )
                .at_offset(saturating_u64(self.data.len())),
            );
            self.accounted.add(0, self.data.len());
            return self.finish(None);
        }
        self.accounted.add(0, OUTER_HEADER_LEN);

        let mut cursor = OUTER_HEADER_LEN;
        while cursor + MARKER.len() <= self.data.len() {
            let Some(relative) = self.data[cursor..]
                .windows(MARKER.len())
                .position(|window| window == MARKER)
            else {
                break;
            };
            let offset = cursor + relative;
            self.marker_count = self.marker_count.saturating_add(1);
            if self.marker_count > self.limits.max_stream_count {
                self.fatal = true;
                self.diagnostics.push(
                    Diagnostic::new(
                        "limit.stream_count",
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Fatal,
                        "modern marker count exceeds the configured stream limit",
                    )
                    .at_offset(saturating_u64(offset))
                    .with_detail("limit", self.limits.max_stream_count.to_string()),
                );
                break;
            }

            match self.classify(offset) {
                MarkerOutcome::Consumed(end) => cursor = end.max(offset + 1),
                MarkerOutcome::Advance => cursor = offset + 1,
                MarkerOutcome::Fatal => {
                    self.fatal = true;
                    break;
                }
            }
        }

        if self.entries.is_empty() && !self.fatal {
            self.diagnostics.push(
                Diagnostic::new(
                    "modern.no_valid_frames",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "modern marker candidate contains no valid container frames",
                )
                .at_offset(OUTER_HEADER_LEN as u64),
            );
        }

        self.entries
            .sort_by_key(|entry| entry.source_range.map_or(u64::MAX, |range| range.offset));
        let format_version = u32_be(self.data, 4).map(u64::from);
        self.finish(format_version)
    }

    fn classify(&mut self, offset: usize) -> MarkerOutcome {
        let Some(header_end) = offset.checked_add(FRAME_HEADER_LEN) else {
            return self.reject_overflow(offset);
        };
        if header_end > self.data.len() {
            self.accounted.add(offset, self.data.len());
            self.diagnostics.push(
                Diagnostic::new(
                    "modern.truncated_frame_header",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "modern marker is truncated inside its frame header",
                )
                .at_offset(saturating_u64(offset)),
            );
            return MarkerOutcome::Consumed(self.data.len());
        }

        let Some(type_id) = u32_le(self.data, offset + 6) else {
            return MarkerOutcome::Advance;
        };
        let Some(field_10) = u32_le(self.data, offset + 10) else {
            return MarkerOutcome::Advance;
        };
        let Some(field_14) = u32_le(self.data, offset + 14) else {
            return MarkerOutcome::Advance;
        };
        let Some(field_18) = u32_le(self.data, offset + 18) else {
            return MarkerOutcome::Advance;
        };
        let Some(name_len_u32) = u32_le(self.data, offset + 22) else {
            return MarkerOutcome::Advance;
        };
        let name_len_u64 = u64::from(name_len_u32);
        if name_len_u64 > self.limits.max_string_bytes {
            self.diagnostics.push(
                Diagnostic::new(
                    "limit.string_bytes",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "modern frame name exceeds the configured string limit",
                )
                .at_offset(saturating_u64(offset + 22))
                .with_detail("declared_bytes", name_len_u64.to_string())
                .with_detail("limit_bytes", self.limits.max_string_bytes.to_string()),
            );
            return MarkerOutcome::Fatal;
        }
        let Some(name_len) = usize_from_u64(name_len_u64) else {
            return self.reject_numeric_range(offset + 22);
        };

        let block_attempt = self.try_block(offset, type_id, field_10, field_14, field_18, name_len);
        if let BlockAttempt::Valid(end) = block_attempt {
            return MarkerOutcome::Consumed(end);
        }

        if let Some(end) =
            self.try_cache_cell(offset, type_id, field_10, field_14, field_18, name_len)
        {
            return MarkerOutcome::Consumed(end);
        }
        if let Some(end) =
            self.try_directory(offset, type_id, field_10, field_14, field_18, name_len)
        {
            return MarkerOutcome::Consumed(end);
        }

        match block_attempt {
            BlockAttempt::Valid(_) => unreachable!("valid block returned above"),
            BlockAttempt::Rejected(diagnostic) => {
                let fatal = diagnostic.kind == DiagnosticKind::Fatal;
                self.diagnostics.push(diagnostic);
                if fatal {
                    MarkerOutcome::Fatal
                } else {
                    MarkerOutcome::Advance
                }
            }
            BlockAttempt::NotPlausible => MarkerOutcome::Advance,
        }
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn try_block(
        &mut self,
        offset: usize,
        type_id: u32,
        expected_crc32: u32,
        compressed_size: u32,
        uncompressed_size: u32,
        name_len: usize,
    ) -> BlockAttempt {
        let Some(payload_offset) = checked_end(offset, &[FRAME_HEADER_LEN, name_len]) else {
            return BlockAttempt::Rejected(Self::numeric_range_diagnostic(offset));
        };
        if payload_offset > self.data.len() {
            return BlockAttempt::Rejected(
                Diagnostic::new(
                    "modern.truncated_block_name",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "modern block preamble extends beyond the input",
                )
                .at_offset(saturating_u64(offset + FRAME_HEADER_LEN)),
            );
        }
        let name_start = offset + FRAME_HEADER_LEN;
        let name = decode_name(&self.data[name_start..payload_offset]);
        if name_len > 0 && name.is_none() {
            return BlockAttempt::NotPlausible;
        }
        if compressed_size == 0 {
            return BlockAttempt::NotPlausible;
        }
        let Some(frame_end) = payload_offset.checked_add(compressed_size as usize) else {
            return BlockAttempt::Rejected(Self::numeric_range_diagnostic(offset));
        };
        if payload_offset > self.data.len() || frame_end > self.data.len() {
            return BlockAttempt::Rejected(
                Diagnostic::new(
                    "modern.truncated_block",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "modern compressed block extends beyond the input",
                )
                .at_offset(saturating_u64(offset))
                .with_detail("declared_end", saturating_u64(frame_end).to_string())
                .with_detail("file_bytes", saturating_u64(self.data.len()).to_string()),
            );
        }
        let stored = &self.data[payload_offset..frame_end];
        let expected_size = u64::from(uncompressed_size);
        let decoded =
            match inflate_raw_bounded(stored, expected_size, self.decoded_bytes, self.limits) {
                Ok(decoded) => decoded,
                Err(failure) => {
                    // The shared marker also introduces metadata cells. Give those
                    // classifiers a chance before surfacing this attempted block.
                    return BlockAttempt::Rejected(inflate_diagnostic(
                        failure,
                        saturating_u64(payload_offset),
                        expected_size,
                        u64::from(compressed_size),
                    ));
                }
            };
        let actual_crc32 = crc32(&decoded);
        if actual_crc32 != expected_crc32 {
            return BlockAttempt::Rejected(
                Diagnostic::new(
                    "modern.checksum_mismatch",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "modern block CRC-32 does not match its decompressed payload",
                )
                .at_offset(saturating_u64(offset + 10))
                .with_detail("expected_crc32", format!("{expected_crc32:08x}"))
                .with_detail("actual_crc32", format!("{actual_crc32:08x}")),
            );
        }

        let id = format!("modern:block:{offset:016x}");
        let mut attributes = BTreeMap::new();
        attributes.insert("type_id".to_owned(), type_id.to_string());
        if name.is_none() && name_len > 0 {
            attributes.insert("name_decode".to_owned(), "non_printable".to_owned());
        }
        self.entries.push(InventoryEntry {
            id: id.clone(),
            path: name,
            kind: InventoryEntryKind::Block,
            state: InventoryEntryState::Decoded,
            source_range: Some(ByteRange::new(
                saturating_u64(offset),
                saturating_u64(frame_end - offset),
            )),
            payload_range: Some(ByteRange::new(
                saturating_u64(payload_offset),
                u64::from(compressed_size),
            )),
            stored_size: u64::from(compressed_size),
            decoded_size: Some(expected_size),
            compression: CompressionMethod::DeflateRaw,
            checksum: ChecksumStatus::Verified,
            expected_crc32: Some(expected_crc32),
            stored_sha256: Some(sha256_hex(stored)),
            decoded_sha256: Some(sha256_hex(&decoded)),
            attributes,
        });
        if self.wants(&id) {
            self.stored.insert(id.clone(), stored.to_vec());
            self.decoded.insert(id, decoded);
        }
        self.accounted.add(offset, frame_end);
        self.decoded_bytes = self.decoded_bytes.saturating_add(expected_size);
        self.decoded_streams = self.decoded_streams.saturating_add(1);
        BlockAttempt::Valid(frame_end)
    }

    #[allow(clippy::too_many_arguments)]
    fn try_cache_cell(
        &mut self,
        offset: usize,
        type_id: u32,
        two_l: u32,
        half_l: u32,
        logical_len: u32,
        name_len: usize,
    ) -> Option<usize> {
        if logical_len == 0
            || two_l != logical_len.saturating_mul(2)
            || half_l != logical_len / 2
            || name_len == 0
        {
            return None;
        }
        let end = checked_end(offset, &[FRAME_HEADER_LEN, name_len])?;
        let raw_name = self.data.get(offset + FRAME_HEADER_LEN..end)?;
        let name = decode_name(raw_name)?;
        let id = format!("modern:cache_cell:{offset:016x}");
        let mut attributes = BTreeMap::new();
        attributes.insert("type_id".to_owned(), type_id.to_string());
        attributes.insert("logical_length".to_owned(), logical_len.to_string());
        self.entries.push(InventoryEntry {
            id,
            path: Some(name),
            kind: InventoryEntryKind::CacheCell,
            state: InventoryEntryState::MetadataOnly,
            source_range: Some(ByteRange::new(
                saturating_u64(offset),
                saturating_u64(end - offset),
            )),
            payload_range: None,
            stored_size: 0,
            decoded_size: None,
            compression: CompressionMethod::None,
            checksum: ChecksumStatus::NotPresent,
            expected_crc32: None,
            stored_sha256: None,
            decoded_sha256: None,
            attributes,
        });
        self.accounted.add(offset, end);
        Some(end)
    }

    fn try_directory(
        &mut self,
        offset: usize,
        type_id: u32,
        zero_1: u32,
        size: u32,
        zero_2: u32,
        name_len: usize,
    ) -> Option<usize> {
        if zero_1 != 0 || zero_2 != 0 || name_len == 0 {
            return None;
        }
        let name_start = offset.checked_add(DIRECTORY_PREFIX_LEN)?;
        let name_end = name_start.checked_add(name_len)?;
        let end = name_end.checked_add(DIRECTORY_TRAILER_LEN)?;
        let raw_name = self.data.get(name_start..name_end)?;
        let trailer = self.data.get(name_end..end)?;
        if trailer.get(4..) != Some(&[0, 0]) {
            return None;
        }
        let name = decode_name(raw_name)?;
        let id = format!("modern:directory_entry:{offset:016x}");
        let mut attributes = BTreeMap::new();
        attributes.insert("type_id".to_owned(), type_id.to_string());
        attributes.insert("section_size".to_owned(), size.to_string());
        attributes.insert(
            "descriptor_sha256".to_owned(),
            sha256_hex(&self.data[offset + 26..name_start]),
        );
        attributes.insert("trailer_hex".to_owned(), hex(trailer));
        self.entries.push(InventoryEntry {
            id,
            path: Some(name),
            kind: InventoryEntryKind::DirectoryEntry,
            state: InventoryEntryState::MetadataOnly,
            source_range: Some(ByteRange::new(
                saturating_u64(offset),
                saturating_u64(end - offset),
            )),
            payload_range: None,
            stored_size: u64::from(size),
            decoded_size: None,
            compression: CompressionMethod::None,
            checksum: ChecksumStatus::NotPresent,
            expected_crc32: None,
            stored_sha256: None,
            decoded_sha256: None,
            attributes,
        });
        self.accounted.add(offset, end);
        Some(end)
    }

    fn reject_overflow(&mut self, offset: usize) -> MarkerOutcome {
        self.diagnostics
            .push(Self::numeric_range_diagnostic(offset));
        MarkerOutcome::Fatal
    }

    fn wants(&self, entry_id: &str) -> bool {
        self.wanted_entry == Some(entry_id)
            || self
                .wanted_entries
                .is_some_and(|entries| entries.contains(entry_id))
    }

    fn reject_numeric_range(&mut self, offset: usize) -> MarkerOutcome {
        self.diagnostics
            .push(Self::numeric_range_diagnostic(offset));
        MarkerOutcome::Fatal
    }

    fn numeric_range_diagnostic(offset: usize) -> Diagnostic {
        Diagnostic::new(
            "limit.numeric_range",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "declared modern frame range cannot be represented safely",
        )
        .at_offset(saturating_u64(offset))
    }

    fn finish(self, format_version: Option<u64>) -> ScanArtifacts {
        let uninterpreted_ranges = self.accounted.complement(self.data.len());
        let uninterpreted_bytes = range_sum(&uninterpreted_ranges);
        let status = if self.fatal {
            InventoryStatus::Rejected
        } else if self.entries.is_empty() {
            InventoryStatus::Malformed
        } else if self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            InventoryStatus::Partial
        } else {
            InventoryStatus::Complete
        };
        let streams_total = self
            .entries
            .iter()
            .filter(|entry| entry.kind == InventoryEntryKind::Block)
            .count();
        let result = InventoryResult {
            status,
            inventory: Some(ContainerInventory {
                envelope: Envelope::ModernChunk,
                format_version,
                entries: self.entries,
                attributes: BTreeMap::from([(
                    "outer_header_bytes".to_owned(),
                    OUTER_HEADER_LEN.to_string(),
                )]),
            }),
            diagnostics: self.diagnostics,
            coverage: CoverageReport {
                total_bytes: saturating_u64(self.data.len()),
                inspected_bytes: saturating_u64(self.data.len()),
                decoded_bytes: self.decoded_bytes,
                uninterpreted_bytes,
                streams_total: saturating_u64(streams_total),
                streams_decoded: self.decoded_streams,
            },
            uninterpreted_ranges,
        };
        ScanArtifacts {
            result,
            stored: self.stored,
            decoded: self.decoded,
        }
    }
}

enum MarkerOutcome {
    Consumed(usize),
    Advance,
    Fatal,
}

enum BlockAttempt {
    Valid(usize),
    Rejected(Diagnostic),
    NotPlausible,
}

fn decode_name(raw: &[u8]) -> Option<String> {
    let mut output = String::with_capacity(raw.len());
    for value in raw {
        let decoded = value.rotate_left(4);
        if !(0x20..0x7f).contains(&decoded) {
            return None;
        }
        output.push(char::from(decoded));
    }
    Some(output)
}

fn hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(data.len() * 2);
    for byte in data {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use crc32fast::hash as crc32;
    use flate2::{Compression, write::DeflateEncoder};
    use sldkit_core::{ChecksumStatus, InventoryStatus, ResourceLimits};
    use std::io::Write;

    use super::{MARKER, scan};

    fn encode_name(value: &str) -> Vec<u8> {
        value.bytes().map(|byte| byte.rotate_left(4)).collect()
    }

    fn block(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        assert!(encoder.write_all(payload).is_ok());
        let compressed = encoder.finish().unwrap_or_default();
        let name = encode_name(name);
        let mut output = Vec::new();
        output.extend_from_slice(MARKER);
        output.extend_from_slice(&7_u32.to_le_bytes());
        output.extend_from_slice(&crc32(payload).to_le_bytes());
        output.extend_from_slice(
            &u32::try_from(compressed.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(
            &u32::try_from(payload.len())
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        output.extend_from_slice(&u32::try_from(name.len()).unwrap_or(u32::MAX).to_le_bytes());
        output.extend_from_slice(&name);
        output.extend_from_slice(&compressed);
        output
    }

    #[test]
    fn validates_and_extracts_a_modern_block() {
        let mut data = vec![1, 2, 3, 4, 0, 0, 0, 4];
        data.extend_from_slice(&block("PreviewPNG", b"payload"));
        let scanned = scan(&data, &ResourceLimits::service(), None);
        assert_eq!(scanned.result.status, InventoryStatus::Complete);
        let inventory = scanned.result.inventory.as_ref();
        assert!(inventory.is_some());
        if let Some(inventory) = inventory {
            assert_eq!(inventory.entries.len(), 1);
            assert_eq!(inventory.entries[0].path.as_deref(), Some("PreviewPNG"));
            assert_eq!(inventory.entries[0].checksum, ChecksumStatus::Verified);
        }
    }

    #[test]
    fn reports_checksum_corruption_without_panicking() {
        let mut data = vec![1, 2, 3, 4, 0, 0, 0, 4];
        let mut encoded = block("Contents/Test", b"payload");
        encoded[10] ^= 1;
        data.extend_from_slice(&encoded);
        let scanned = scan(&data, &ResourceLimits::service(), None);
        assert_eq!(scanned.result.status, InventoryStatus::Malformed);
        assert!(
            scanned
                .result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "modern.checksum_mismatch")
        );
    }

    #[test]
    fn rejects_declared_compression_bomb_before_allocating() {
        let mut limits = ResourceLimits::service();
        limits.max_compression_ratio = 2;
        let mut data = vec![1, 2, 3, 4, 0, 0, 0, 4];
        let mut encoded = block("Contents/Test", b"payload");
        encoded[18..22].copy_from_slice(&1_000_u32.to_le_bytes());
        data.extend_from_slice(&encoded);
        let scanned = scan(&data, &limits, None);
        assert_eq!(scanned.result.status, InventoryStatus::Rejected);
        assert!(
            scanned
                .result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "limit.compression_ratio")
        );
    }

    #[test]
    fn reports_truncated_payload_at_its_frame_offset() {
        let mut data = vec![1, 2, 3, 4, 0, 0, 0, 4];
        let mut encoded = block("Contents/Test", b"payload");
        encoded.pop();
        data.extend_from_slice(&encoded);
        let scanned = scan(&data, &ResourceLimits::service(), None);
        assert_eq!(scanned.result.status, InventoryStatus::Malformed);
        assert!(
            scanned
                .result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "modern.truncated_block"
                    && diagnostic.offset == Some(8))
        );
    }
}
