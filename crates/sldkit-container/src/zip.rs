use std::collections::{BTreeMap, BTreeSet};

use crc32fast::hash as crc32;
use sldkit_core::{
    ByteRange, ChecksumStatus, CompressionMethod, ContainerInventory, CoverageReport, Diagnostic,
    DiagnosticKind, DiagnosticSeverity, Envelope, InventoryEntry, InventoryEntryKind,
    InventoryEntryState, InventoryResult, InventoryStatus, ResourceLimits,
};

use crate::common::{
    RangeSet, ScanArtifacts, checked_end, range_sum, saturating_u64, sha256_hex, u16_le, u32_le,
};

const LOCAL_SIGNATURE: &[u8; 4] = b"PK\x03\x04";
const CENTRAL_SIGNATURE: &[u8; 4] = b"PK\x01\x02";
const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
const DATA_DESCRIPTOR_SIGNATURE: &[u8; 4] = b"PK\x07\x08";
const EOCD_LEN: usize = 22;
const CENTRAL_LEN: usize = 46;
const LOCAL_LEN: usize = 30;
const MAX_EOCD_SEARCH: usize = EOCD_LEN + u16::MAX as usize;

#[derive(Clone, Copy)]
struct Eocd {
    offset: usize,
    end: usize,
    entry_count: u16,
    central_offset: u32,
    central_size: u32,
}

#[derive(Debug)]
struct ZipScanner<'a> {
    data: &'a [u8],
    limits: &'a ResourceLimits,
    wanted_entry: Option<&'a str>,
    wanted_entries: Option<&'a BTreeSet<String>>,
    entries: Vec<InventoryEntry>,
    diagnostics: Vec<Diagnostic>,
    stored: BTreeMap<String, Vec<u8>>,
    decoded: BTreeMap<String, Vec<u8>>,
    accounted: RangeSet,
    decoded_bytes: u64,
    decoded_streams: u64,
    malformed: bool,
    fatal: bool,
    unsupported_container: bool,
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
    let scanner = ZipScanner {
        data,
        limits,
        wanted_entry,
        wanted_entries,
        entries: Vec::new(),
        diagnostics: Vec::new(),
        stored: BTreeMap::new(),
        decoded: BTreeMap::new(),
        accounted: RangeSet::default(),
        decoded_bytes: 0,
        decoded_streams: 0,
        malformed: false,
        fatal: false,
        unsupported_container: false,
    };
    scanner.walk()
}

impl ZipScanner<'_> {
    #[allow(clippy::too_many_lines)]
    fn walk(mut self) -> ScanArtifacts {
        let eocd = match find_eocd(self.data) {
            Ok(eocd) => eocd,
            Err(diagnostic) => {
                let status = if diagnostic.kind == DiagnosticKind::Unsupported {
                    InventoryStatus::Unsupported
                } else {
                    InventoryStatus::Malformed
                };
                self.diagnostics.push(diagnostic);
                return self.finish(None, status);
            }
        };
        self.accounted.add(eocd.offset, eocd.end);

        if eocd.entry_count == u16::MAX
            || eocd.central_offset == u32::MAX
            || eocd.central_size == u32::MAX
        {
            self.diagnostics.push(
                Diagnostic::new(
                    "zip.zip64_unsupported",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Unsupported,
                    "ZIP64 metadata is detected but not supported by the current inventory",
                )
                .at_offset(saturating_u64(eocd.offset)),
            );
            return self.finish(None, InventoryStatus::Unsupported);
        }
        if u64::from(eocd.entry_count) > self.limits.max_stream_count {
            self.diagnostics.push(
                Diagnostic::new(
                    "limit.stream_count",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "ZIP entry count exceeds the configured stream limit",
                )
                .at_offset(saturating_u64(eocd.offset + 10))
                .with_detail("actual", eocd.entry_count.to_string())
                .with_detail("limit", self.limits.max_stream_count.to_string()),
            );
            return self.finish(None, InventoryStatus::Rejected);
        }

        let central_start = eocd.central_offset as usize;
        let Some(central_end) = central_start.checked_add(eocd.central_size as usize) else {
            return self.numeric_failure(16);
        };
        if central_end > eocd.offset || central_end > self.data.len() {
            self.diagnostics.push(
                Diagnostic::new(
                    "zip.invalid_central_range",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP central directory extends beyond its EOCD boundary",
                )
                .at_offset(saturating_u64(eocd.offset + 16))
                .with_detail("central_end", saturating_u64(central_end).to_string())
                .with_detail("eocd_offset", saturating_u64(eocd.offset).to_string()),
            );
            return self.finish(None, InventoryStatus::Malformed);
        }

        let mut cursor = central_start;
        let mut declared_uncompressed = 0_u64;
        for index in 0..usize::from(eocd.entry_count) {
            match self.read_entry(index, cursor, &mut declared_uncompressed) {
                Ok(next) => cursor = next,
                Err(EntryFailure::Diagnostic(diagnostic)) => {
                    let fatal = diagnostic.kind == DiagnosticKind::Fatal;
                    let unsupported = diagnostic.kind == DiagnosticKind::Unsupported;
                    self.diagnostics.push(diagnostic);
                    if fatal {
                        self.fatal = true;
                    } else if unsupported {
                        self.unsupported_container = true;
                    } else {
                        self.malformed = true;
                    }
                    break;
                }
                Err(EntryFailure::Numeric(offset)) => return self.numeric_failure(offset),
            }
        }
        if !self.fatal && cursor != central_end {
            self.malformed = true;
            self.diagnostics.push(
                Diagnostic::new(
                    "zip.central_size_mismatch",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "parsed central-directory extent differs from the EOCD declaration",
                )
                .at_offset(saturating_u64(cursor))
                .with_detail("parsed_end", saturating_u64(cursor).to_string())
                .with_detail("declared_end", saturating_u64(central_end).to_string()),
            );
        }

        let status = if self.fatal {
            InventoryStatus::Rejected
        } else if self.unsupported_container {
            InventoryStatus::Unsupported
        } else if self.malformed {
            if self.entries.is_empty() {
                InventoryStatus::Malformed
            } else {
                InventoryStatus::Partial
            }
        } else {
            InventoryStatus::Complete
        };
        self.finish(None, status)
    }

    #[allow(clippy::too_many_lines)]
    fn read_entry(
        &mut self,
        index: usize,
        central_offset: usize,
        declared_uncompressed: &mut u64,
    ) -> Result<usize, EntryFailure> {
        let Some(fixed_end) = central_offset.checked_add(CENTRAL_LEN) else {
            return Err(EntryFailure::Numeric(central_offset));
        };
        if fixed_end > self.data.len()
            || self.data.get(central_offset..central_offset + 4) != Some(CENTRAL_SIGNATURE)
        {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.invalid_central_entry",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP central-directory entry is missing or truncated",
                )
                .at_offset(saturating_u64(central_offset)),
            ));
        }

        let flags = u16_le(self.data, central_offset + 8).unwrap_or(0);
        let method = u16_le(self.data, central_offset + 10).unwrap_or(u16::MAX);
        let expected_crc = u32_le(self.data, central_offset + 16).unwrap_or(0);
        let compressed_size = u32_le(self.data, central_offset + 20).unwrap_or(u32::MAX);
        let uncompressed_size = u32_le(self.data, central_offset + 24).unwrap_or(u32::MAX);
        let name_len = usize::from(u16_le(self.data, central_offset + 28).unwrap_or(0));
        let extra_len = usize::from(u16_le(self.data, central_offset + 30).unwrap_or(0));
        let comment_len = usize::from(u16_le(self.data, central_offset + 32).unwrap_or(0));
        let disk_start = u16_le(self.data, central_offset + 34).unwrap_or(u16::MAX);
        let local_offset = u32_le(self.data, central_offset + 42).unwrap_or(u32::MAX) as usize;
        if disk_start != 0 {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.multidisk_unsupported",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Unsupported,
                    "multi-disk ZIP archives are not supported",
                )
                .at_offset(saturating_u64(central_offset + 34)),
            ));
        }
        if compressed_size == u32::MAX
            || uncompressed_size == u32::MAX
            || local_offset == u32::MAX as usize
        {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.zip64_unsupported",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Unsupported,
                    "ZIP64 entry metadata is not supported by the current inventory",
                )
                .at_offset(saturating_u64(central_offset)),
            ));
        }
        if saturating_u64(name_len) > self.limits.max_string_bytes {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "limit.string_bytes",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "ZIP entry name exceeds the configured string limit",
                )
                .at_offset(saturating_u64(central_offset + 28))
                .with_detail("actual_bytes", name_len.to_string())
                .with_detail("limit_bytes", self.limits.max_string_bytes.to_string()),
            ));
        }
        let Some(central_end) = checked_end(
            central_offset,
            &[CENTRAL_LEN, name_len, extra_len, comment_len],
        ) else {
            return Err(EntryFailure::Numeric(central_offset));
        };
        let Some(name_bytes) = self.data.get(fixed_end..fixed_end + name_len) else {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.truncated_central_entry",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP central-directory variable fields extend beyond the input",
                )
                .at_offset(saturating_u64(fixed_end)),
            ));
        };
        if central_end > self.data.len() {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.truncated_central_entry",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP central-directory variable fields extend beyond the input",
                )
                .at_offset(saturating_u64(fixed_end)),
            ));
        }
        self.accounted.add(central_offset, central_end);

        let (path, path_encoding) = decode_name(name_bytes, flags);
        let depth = path
            .split('/')
            .filter(|component| !component.is_empty())
            .count();
        if u32::try_from(depth).unwrap_or(u32::MAX) > self.limits.max_nesting_depth {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "limit.nesting_depth",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "ZIP entry path exceeds the configured nesting limit",
                )
                .at_offset(saturating_u64(fixed_end))
                .in_stream(path)
                .with_detail("actual", depth.to_string())
                .with_detail("limit", self.limits.max_nesting_depth.to_string()),
            ));
        }
        let unsafe_path = is_unsafe_path(&path);
        if unsafe_path {
            self.diagnostics.push(
                Diagnostic::new(
                    "zip.unsafe_path",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Preserved,
                    "ZIP entry path would be unsafe for filesystem extraction",
                )
                .at_offset(saturating_u64(fixed_end))
                .in_stream(path.clone()),
            );
        }

        *declared_uncompressed = declared_uncompressed.saturating_add(u64::from(uncompressed_size));
        if *declared_uncompressed > self.limits.max_total_uncompressed_bytes {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "limit.uncompressed_bytes",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "aggregate ZIP uncompressed size exceeds the configured limit",
                )
                .at_offset(saturating_u64(central_offset + 24))
                .in_stream(path)
                .with_detail("declared_bytes", declared_uncompressed.to_string())
                .with_detail(
                    "limit_bytes",
                    self.limits.max_total_uncompressed_bytes.to_string(),
                ),
            ));
        }
        let compressed_u64 = u64::from(compressed_size);
        let uncompressed_u64 = u64::from(uncompressed_size);
        if uncompressed_u64 > 0
            && (compressed_u64 == 0
                || uncompressed_u64
                    > compressed_u64.saturating_mul(self.limits.max_compression_ratio))
        {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "limit.compression_ratio",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "ZIP entry declared compression ratio exceeds the configured limit",
                )
                .at_offset(saturating_u64(central_offset + 20))
                .in_stream(path)
                .with_detail("compressed_bytes", compressed_size.to_string())
                .with_detail("uncompressed_bytes", uncompressed_size.to_string()),
            ));
        }

        let local = self.read_local(
            local_offset,
            flags,
            method,
            name_bytes,
            compressed_size,
            uncompressed_size,
            expected_crc,
        )?;
        self.accounted.add(local_offset, local.frame_end);
        let stored_bytes = &self.data[local.payload_offset..local.payload_end];
        let id = format!("zip:entry:{index:08x}:{central_offset:016x}");
        let stored_hash = sha256_hex(stored_bytes);
        let encrypted = flags & 1 != 0;
        let (state, compression, checksum, decoded_hash) = if encrypted {
            self.diagnostics.push(
                Diagnostic::new(
                    "zip.encryption_unsupported",
                    DiagnosticSeverity::Warning,
                    DiagnosticKind::Unsupported,
                    "encrypted ZIP entry payload is not decoded",
                )
                .at_offset(saturating_u64(central_offset + 8))
                .in_stream(path.clone()),
            );
            (
                InventoryEntryState::Unsupported,
                compression_method(method),
                ChecksumStatus::NotChecked,
                None,
            )
        } else if method == 0 {
            let actual_crc = crc32(stored_bytes);
            if compressed_size != uncompressed_size || actual_crc != expected_crc {
                self.malformed = true;
                self.diagnostics.push(
                    Diagnostic::new(
                        "zip.checksum_or_size_mismatch",
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Malformed,
                        "stored ZIP entry does not match its central-directory size or CRC-32",
                    )
                    .at_offset(saturating_u64(local.payload_offset))
                    .in_stream(path.clone())
                    .with_detail("expected_crc32", format!("{expected_crc:08x}"))
                    .with_detail("actual_crc32", format!("{actual_crc:08x}")),
                );
                (
                    InventoryEntryState::Malformed,
                    CompressionMethod::ZipStored,
                    ChecksumStatus::Mismatch,
                    None,
                )
            } else {
                self.decoded_bytes = self.decoded_bytes.saturating_add(uncompressed_u64);
                self.decoded_streams = self.decoded_streams.saturating_add(1);
                if self.wants(&id) {
                    self.decoded.insert(id.clone(), stored_bytes.to_vec());
                }
                (
                    InventoryEntryState::Decoded,
                    CompressionMethod::ZipStored,
                    ChecksumStatus::Verified,
                    Some(stored_hash.clone()),
                )
            }
        } else {
            self.diagnostics.push(
                Diagnostic::new(
                    "zip.payload_decode_unsupported",
                    DiagnosticSeverity::Info,
                    DiagnosticKind::Unsupported,
                    "this ZIP entry compression method is not supported for payload decoding",
                )
                .at_offset(saturating_u64(central_offset + 10))
                .in_stream(path.clone())
                .with_detail("method", method.to_string()),
            );
            (
                InventoryEntryState::Unsupported,
                compression_method(method),
                ChecksumStatus::NotChecked,
                None,
            )
        };
        if self.wants(&id) {
            self.stored.insert(id.clone(), stored_bytes.to_vec());
        }
        let mut attributes = BTreeMap::new();
        attributes.insert("flags".to_owned(), flags.to_string());
        attributes.insert("zip_method".to_owned(), method.to_string());
        attributes.insert("path_encoding".to_owned(), path_encoding.to_owned());
        attributes.insert("unsafe_path".to_owned(), unsafe_path.to_string());
        attributes.insert("local_header_offset".to_owned(), local_offset.to_string());
        self.entries.push(InventoryEntry {
            id,
            path: Some(path),
            kind: InventoryEntryKind::ZipEntry,
            state,
            source_range: Some(ByteRange::new(
                saturating_u64(local_offset),
                saturating_u64(local.frame_end - local_offset),
            )),
            payload_range: Some(ByteRange::new(
                saturating_u64(local.payload_offset),
                compressed_u64,
            )),
            stored_size: compressed_u64,
            decoded_size: Some(uncompressed_u64),
            compression,
            checksum,
            expected_crc32: Some(expected_crc),
            stored_sha256: Some(stored_hash),
            decoded_sha256: decoded_hash,
            attributes,
        });
        Ok(central_end)
    }

    fn wants(&self, entry_id: &str) -> bool {
        self.wanted_entry == Some(entry_id)
            || self
                .wanted_entries
                .is_some_and(|entries| entries.contains(entry_id))
    }

    #[allow(clippy::too_many_arguments)]
    fn read_local(
        &self,
        offset: usize,
        central_flags: u16,
        central_method: u16,
        central_name: &[u8],
        compressed_size: u32,
        uncompressed_size: u32,
        expected_crc: u32,
    ) -> Result<LocalFacts, EntryFailure> {
        let Some(fixed_end) = offset.checked_add(LOCAL_LEN) else {
            return Err(EntryFailure::Numeric(offset));
        };
        if fixed_end > self.data.len() || self.data.get(offset..offset + 4) != Some(LOCAL_SIGNATURE)
        {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.invalid_local_header",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP local file header is missing or truncated",
                )
                .at_offset(saturating_u64(offset)),
            ));
        }
        let local_flags = u16_le(self.data, offset + 6).unwrap_or(u16::MAX);
        let local_method = u16_le(self.data, offset + 8).unwrap_or(u16::MAX);
        if local_flags != central_flags || local_method != central_method {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.local_central_mismatch",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP local and central headers disagree on flags or compression",
                )
                .at_offset(saturating_u64(offset + 6)),
            ));
        }
        let name_len = usize::from(u16_le(self.data, offset + 26).unwrap_or(0));
        let extra_len = usize::from(u16_le(self.data, offset + 28).unwrap_or(0));
        let Some(payload_offset) = checked_end(offset, &[LOCAL_LEN, name_len, extra_len]) else {
            return Err(EntryFailure::Numeric(offset));
        };
        if self.data.get(fixed_end..fixed_end + name_len) != Some(central_name) {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.local_name_mismatch",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP local and central headers contain different entry names",
                )
                .at_offset(saturating_u64(fixed_end)),
            ));
        }
        let Some(payload_end) = payload_offset.checked_add(compressed_size as usize) else {
            return Err(EntryFailure::Numeric(payload_offset));
        };
        if payload_end > self.data.len() {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.truncated_payload",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP entry payload extends beyond the input",
                )
                .at_offset(saturating_u64(payload_offset)),
            ));
        }

        let frame_end = if central_flags & 0x0008 == 0 {
            let local_crc = u32_le(self.data, offset + 14).unwrap_or(0);
            let local_compressed = u32_le(self.data, offset + 18).unwrap_or(0);
            let local_uncompressed = u32_le(self.data, offset + 22).unwrap_or(0);
            if local_crc != expected_crc
                || local_compressed != compressed_size
                || local_uncompressed != uncompressed_size
            {
                return Err(EntryFailure::Diagnostic(
                    Diagnostic::new(
                        "zip.local_central_size_mismatch",
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Malformed,
                        "ZIP local and central headers disagree on CRC or sizes",
                    )
                    .at_offset(saturating_u64(offset + 14)),
                ));
            }
            payload_end
        } else {
            self.read_descriptor(
                payload_end,
                expected_crc,
                compressed_size,
                uncompressed_size,
            )?
        };
        Ok(LocalFacts {
            payload_offset,
            payload_end,
            frame_end,
        })
    }

    fn read_descriptor(
        &self,
        offset: usize,
        expected_crc: u32,
        compressed_size: u32,
        uncompressed_size: u32,
    ) -> Result<usize, EntryFailure> {
        let has_signature =
            self.data.get(offset..offset.saturating_add(4)) == Some(DATA_DESCRIPTOR_SIGNATURE);
        let values_offset = if has_signature {
            offset.checked_add(4)
        } else {
            Some(offset)
        }
        .ok_or(EntryFailure::Numeric(offset))?;
        let end = values_offset
            .checked_add(12)
            .ok_or(EntryFailure::Numeric(values_offset))?;
        if end > self.data.len()
            || u32_le(self.data, values_offset) != Some(expected_crc)
            || u32_le(self.data, values_offset + 4) != Some(compressed_size)
            || u32_le(self.data, values_offset + 8) != Some(uncompressed_size)
        {
            return Err(EntryFailure::Diagnostic(
                Diagnostic::new(
                    "zip.invalid_data_descriptor",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Malformed,
                    "ZIP data descriptor is missing or disagrees with the central directory",
                )
                .at_offset(saturating_u64(offset)),
            ));
        }
        Ok(end)
    }

    fn numeric_failure(mut self, offset: usize) -> ScanArtifacts {
        self.fatal = true;
        self.diagnostics.push(
            Diagnostic::new(
                "limit.numeric_range",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "ZIP byte range cannot be represented safely",
            )
            .at_offset(saturating_u64(offset)),
        );
        self.finish(None, InventoryStatus::Rejected)
    }

    fn finish(self, format_version: Option<u64>, status: InventoryStatus) -> ScanArtifacts {
        let uninterpreted_ranges = self.accounted.complement(self.data.len());
        let uninterpreted_bytes = range_sum(&uninterpreted_ranges);
        let streams_total = saturating_u64(self.entries.len());
        let result = InventoryResult {
            status,
            inventory: Some(ContainerInventory {
                envelope: Envelope::ZipOpc,
                format_version,
                entries: self.entries,
                attributes: BTreeMap::from([(
                    "content_semantics".to_owned(),
                    "detection_only".to_owned(),
                )]),
            }),
            diagnostics: self.diagnostics,
            coverage: CoverageReport {
                total_bytes: saturating_u64(self.data.len()),
                inspected_bytes: saturating_u64(self.data.len()),
                decoded_bytes: self.decoded_bytes,
                uninterpreted_bytes,
                streams_total,
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

struct LocalFacts {
    payload_offset: usize,
    payload_end: usize,
    frame_end: usize,
}

enum EntryFailure {
    Diagnostic(Diagnostic),
    Numeric(usize),
}

fn find_eocd(data: &[u8]) -> Result<Eocd, Diagnostic> {
    if data.len() < EOCD_LEN {
        return Err(Diagnostic::new(
            "zip.missing_eocd",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "ZIP input is too short to contain an end-of-central-directory record",
        )
        .at_offset(saturating_u64(data.len())));
    }
    let start = data.len().saturating_sub(MAX_EOCD_SEARCH);
    for offset in (start..=data.len() - EOCD_LEN).rev() {
        if data.get(offset..offset + 4) != Some(EOCD_SIGNATURE) {
            continue;
        }
        let disk = u16_le(data, offset + 4).unwrap_or(u16::MAX);
        let central_disk = u16_le(data, offset + 6).unwrap_or(u16::MAX);
        let entries_on_disk = u16_le(data, offset + 8).unwrap_or(u16::MAX);
        let entry_count = u16_le(data, offset + 10).unwrap_or(u16::MAX);
        let comment_len = usize::from(u16_le(data, offset + 20).unwrap_or(u16::MAX));
        let Some(end) = offset.checked_add(EOCD_LEN + comment_len) else {
            continue;
        };
        if end > data.len() {
            continue;
        }
        if disk != 0 || central_disk != 0 || entries_on_disk != entry_count {
            return Err(Diagnostic::new(
                "zip.multidisk_unsupported",
                DiagnosticSeverity::Error,
                DiagnosticKind::Unsupported,
                "multi-disk ZIP archives are not supported",
            )
            .at_offset(saturating_u64(offset + 4)));
        }
        return Ok(Eocd {
            offset,
            end,
            entry_count,
            central_offset: u32_le(data, offset + 16).unwrap_or(u32::MAX),
            central_size: u32_le(data, offset + 12).unwrap_or(u32::MAX),
        });
    }
    Err(Diagnostic::new(
        "zip.missing_eocd",
        DiagnosticSeverity::Error,
        DiagnosticKind::Malformed,
        "ZIP end-of-central-directory record was not found within the bounded search window",
    )
    .at_offset(saturating_u64(start)))
}

fn decode_name(raw: &[u8], flags: u16) -> (String, &'static str) {
    if let Ok(value) = std::str::from_utf8(raw) {
        return (
            value.to_owned(),
            if flags & 0x0800 != 0 {
                "utf8"
            } else {
                "ascii_or_utf8"
            },
        );
    }
    let mut output = String::from("bytes:");
    for byte in raw {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    (output, "raw_hex")
}

fn is_unsafe_path(path: &str) -> bool {
    path.starts_with('/')
        || path.starts_with('\\')
        || path.as_bytes().get(1).is_some_and(|value| *value == b':')
        || path.split(['/', '\\']).any(|component| component == "..")
}

const fn compression_method(method: u16) -> CompressionMethod {
    match method {
        0 => CompressionMethod::ZipStored,
        8 => CompressionMethod::ZipDeflate,
        _ => CompressionMethod::Unsupported,
    }
}

#[cfg(test)]
mod tests {
    use crc32fast::hash as crc32;
    use sldkit_core::{ExtractionMode, InventoryStatus, ResourceLimits};

    use super::{CENTRAL_SIGNATURE, EOCD_SIGNATURE, LOCAL_SIGNATURE, scan};

    fn stored_zip(name: &str, payload: &[u8]) -> Vec<u8> {
        let name = name.as_bytes();
        let crc = crc32(payload);
        let payload_len = u32::try_from(payload.len()).unwrap_or(u32::MAX);
        let name_len = u16::try_from(name.len()).unwrap_or(u16::MAX);
        let mut output = Vec::new();
        output.extend_from_slice(LOCAL_SIGNATURE);
        output.extend_from_slice(&20_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&[0; 4]);
        output.extend_from_slice(&crc.to_le_bytes());
        output.extend_from_slice(&payload_len.to_le_bytes());
        output.extend_from_slice(&payload_len.to_le_bytes());
        output.extend_from_slice(&name_len.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(payload);
        let central_offset = u32::try_from(output.len()).unwrap_or(u32::MAX);
        output.extend_from_slice(CENTRAL_SIGNATURE);
        output.extend_from_slice(&20_u16.to_le_bytes());
        output.extend_from_slice(&20_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&[0; 4]);
        output.extend_from_slice(&crc.to_le_bytes());
        output.extend_from_slice(&payload_len.to_le_bytes());
        output.extend_from_slice(&payload_len.to_le_bytes());
        output.extend_from_slice(&name_len.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        output.extend_from_slice(name);
        let central_end = u32::try_from(output.len()).unwrap_or(u32::MAX);
        let central_size = central_end.saturating_sub(central_offset);
        output.extend_from_slice(EOCD_SIGNATURE);
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&1_u16.to_le_bytes());
        output.extend_from_slice(&1_u16.to_le_bytes());
        output.extend_from_slice(&central_size.to_le_bytes());
        output.extend_from_slice(&central_offset.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output
    }

    #[test]
    fn inventories_and_extracts_stored_entry() {
        let data = stored_zip("docProps/test.xml", b"payload");
        let inspected = scan(&data, &ResourceLimits::service(), None);
        assert_eq!(inspected.result.status, InventoryStatus::Complete);
        let entry_id = inspected
            .result
            .inventory
            .as_ref()
            .and_then(|inventory| inventory.entries.first().map(|entry| entry.id.clone()));
        assert!(entry_id.is_some());
        if let Some(entry_id) = entry_id {
            let extracted = scan(&data, &ResourceLimits::service(), Some(&entry_id))
                .extraction(&entry_id, ExtractionMode::Decoded);
            assert_eq!(extracted.data.as_deref(), Some(b"payload".as_slice()));
        }
    }

    #[test]
    fn rejects_declared_zip_bomb() {
        let mut data = stored_zip("bomb", b"x");
        let central = data
            .windows(CENTRAL_SIGNATURE.len())
            .position(|window| window == CENTRAL_SIGNATURE)
            .unwrap_or(0);
        data[central + 24..central + 28].copy_from_slice(&10_000_u32.to_le_bytes());
        let inspected = scan(&data, &ResourceLimits::service(), None);
        assert_eq!(inspected.result.status, InventoryStatus::Rejected);
        assert!(
            inspected
                .result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "limit.compression_ratio")
        );
    }

    #[test]
    fn preserves_unsafe_path_but_never_extracts_to_it() {
        let data = stored_zip("../outside", b"payload");
        let inspected = scan(&data, &ResourceLimits::service(), None);
        assert_eq!(inspected.result.status, InventoryStatus::Complete);
        assert!(
            inspected
                .result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "zip.unsafe_path")
        );
        let unsafe_attribute = inspected.result.inventory.as_ref().and_then(|inventory| {
            inventory
                .entries
                .first()
                .and_then(|entry| entry.attributes.get("unsafe_path"))
        });
        assert_eq!(unsafe_attribute.map(String::as_str), Some("true"));
    }
}
