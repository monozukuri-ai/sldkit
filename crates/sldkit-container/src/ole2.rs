use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
};

use sldkit_core::{
    ChecksumStatus, CompressionMethod, ContainerInventory, CoverageReport, Diagnostic,
    DiagnosticKind, DiagnosticSeverity, Envelope, InventoryEntry, InventoryEntryKind,
    InventoryEntryState, InventoryResult, InventoryStatus, ResourceLimits,
};

use crate::common::{
    InflateFailure, RangeSet, ScanArtifacts, checked_end, inflate_diagnostic, inflate_zlib_bounded,
    range_sum, saturating_u64, sha256_hex, u16_le, u32_le, usize_from_u64,
};

const CFB_HEADER_LEN: usize = 512;
const ZLB_MAGIC: &[u8; 16] = b"\x23\x1d\xd5\x71\xda\x81\x48\xa2\xa8\x58\x98\xb2\x1b\x89\xef\x99";
const ZLB_HEADER_LEN: usize = 24;
const ZLB_TRAILER_LEN: usize = 8;

#[derive(Clone, Copy)]
struct HeaderFacts {
    major_version: u16,
    sector_size: usize,
    physical_sectors: u64,
}

#[derive(Clone)]
struct EntryFacts {
    path: String,
    is_stream: bool,
    is_root: bool,
    len: u64,
    clsid: String,
    state_bits: u32,
}

#[allow(clippy::too_many_lines)]
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

#[allow(clippy::too_many_lines)]
fn scan_impl<'a>(
    data: &'a [u8],
    limits: &'a ResourceLimits,
    wanted_entry: Option<&'a str>,
    wanted_entries: Option<&'a BTreeSet<String>>,
) -> ScanArtifacts {
    let mut accounted = RangeSet::default();
    let header = match preflight(data, &mut accounted) {
        Ok(header) => header,
        Err(diagnostic) => {
            let status = if diagnostic.kind == DiagnosticKind::Unsupported {
                InventoryStatus::Unsupported
            } else {
                InventoryStatus::Malformed
            };
            return failed_scan(data, diagnostic, &accounted, status);
        }
    };

    let mut compound = match cfb::OpenOptions::new()
        .max_buffer_size(64 * 1024)
        .open_with(Cursor::new(data))
    {
        Ok(compound) => compound,
        Err(error) => {
            let diagnostic = Diagnostic::new(
                "ole2.invalid_container",
                DiagnosticSeverity::Error,
                DiagnosticKind::Malformed,
                "CFB validation failed before the storage tree could be walked",
            )
            .at_offset(0)
            .with_detail("error_kind", format!("{:?}", error.kind()))
            .with_detail("failure_stage", "cfb_open");
            return failed_scan(data, diagnostic, &accounted, InventoryStatus::Malformed);
        }
    };

    let mut facts = Vec::new();
    for entry in compound.walk() {
        if saturating_u64(facts.len()) >= limits.max_stream_count {
            let diagnostic = Diagnostic::new(
                "limit.stream_count",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "CFB storage and stream count exceeds the configured limit",
            )
            .at_offset(48)
            .with_detail("limit", limits.max_stream_count.to_string());
            return failed_scan(data, diagnostic, &accounted, InventoryStatus::Rejected);
        }
        facts.push(EntryFacts {
            path: entry.path().to_string_lossy().into_owned(),
            is_stream: entry.is_stream(),
            is_root: entry.is_root(),
            len: entry.len(),
            clsid: entry.clsid().to_string(),
            state_bits: entry.state_bits(),
        });
    }
    facts.sort_by(|left, right| left.path.cmp(&right.path));

    let mut diagnostics = Vec::new();
    let mut fatal = false;
    let mut total_declared = 0_u64;
    for fact in &facts {
        let name_bytes = fact.path.rsplit('/').next().map_or(0, str::len);
        if saturating_u64(name_bytes) > limits.max_string_bytes {
            fatal = true;
            diagnostics.push(
                Diagnostic::new(
                    "limit.string_bytes",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "CFB directory name exceeds the configured string limit",
                )
                .at_offset(48)
                .in_stream(fact.path.clone())
                .with_detail("actual_bytes", name_bytes.to_string())
                .with_detail("limit_bytes", limits.max_string_bytes.to_string()),
            );
            break;
        }
        let depth = fact
            .path
            .split('/')
            .filter(|component| !component.is_empty())
            .count();
        if u32::try_from(depth).unwrap_or(u32::MAX) > limits.max_nesting_depth {
            fatal = true;
            diagnostics.push(
                Diagnostic::new(
                    "limit.nesting_depth",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "CFB storage depth exceeds the configured nesting limit",
                )
                .at_offset(48)
                .in_stream(fact.path.clone())
                .with_detail("actual", depth.to_string())
                .with_detail("limit", limits.max_nesting_depth.to_string()),
            );
            break;
        }
        if fact.is_stream {
            total_declared = total_declared.saturating_add(fact.len);
            if total_declared > limits.max_total_uncompressed_bytes {
                fatal = true;
                diagnostics.push(
                    Diagnostic::new(
                        "limit.uncompressed_bytes",
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Fatal,
                        "aggregate CFB stream length exceeds the configured limit",
                    )
                    .at_offset(48)
                    .in_stream(fact.path.clone())
                    .with_detail("declared_bytes", total_declared.to_string())
                    .with_detail(
                        "limit_bytes",
                        limits.max_total_uncompressed_bytes.to_string(),
                    ),
                );
                break;
            }
        }
    }

    if fatal {
        return finish(
            data,
            header,
            Vec::new(),
            diagnostics,
            BTreeMap::new(),
            BTreeMap::new(),
            0,
            0,
            InventoryStatus::Rejected,
            &accounted,
        );
    }

    let mut entries = Vec::with_capacity(facts.len());
    let mut stored_payloads = BTreeMap::new();
    let mut decoded_payloads = BTreeMap::new();
    let mut decoded_bytes = 0_u64;
    let mut decoded_streams = 0_u64;
    let mut malformed_stream = false;

    for fact in facts {
        let kind = if fact.is_stream {
            InventoryEntryKind::Stream
        } else {
            InventoryEntryKind::Storage
        };
        let id = if fact.is_stream {
            format!("ole2:stream:{}", fact.path)
        } else {
            format!("ole2:storage:{}", fact.path)
        };
        let mut attributes = BTreeMap::new();
        attributes.insert("cfb_fragmented".to_owned(), "true".to_owned());
        attributes.insert("clsid".to_owned(), fact.clsid);
        attributes.insert("state_bits".to_owned(), fact.state_bits.to_string());
        if fact.is_root {
            attributes.insert("root".to_owned(), "true".to_owned());
        }

        if !fact.is_stream {
            entries.push(InventoryEntry {
                id,
                path: Some(fact.path),
                kind,
                state: InventoryEntryState::MetadataOnly,
                source_range: None,
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
            continue;
        }

        let stored = match read_stream(&mut compound, &fact.path, fact.len) {
            Ok(stored) => stored,
            Err(error) => {
                malformed_stream = true;
                diagnostics.push(
                    Diagnostic::new(
                        "ole2.stream_read_failed",
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Malformed,
                        "CFB stream chain could not be read to its declared length",
                    )
                    .at_offset(48)
                    .in_stream(fact.path.clone())
                    .with_detail("error_kind", format!("{:?}", error.kind())),
                );
                entries.push(InventoryEntry {
                    id,
                    path: Some(fact.path),
                    kind,
                    state: InventoryEntryState::Malformed,
                    source_range: None,
                    payload_range: None,
                    stored_size: fact.len,
                    decoded_size: None,
                    compression: CompressionMethod::None,
                    checksum: ChecksumStatus::NotPresent,
                    expected_crc32: None,
                    stored_sha256: None,
                    decoded_sha256: None,
                    attributes,
                });
                continue;
            }
        };

        let stored_hash = sha256_hex(&stored);
        let is_zlb = fact.path.ends_with("__ZLB");
        if is_zlb {
            match decode_zlb(&stored, decoded_bytes, limits) {
                Ok(decoded) => {
                    let decoded_hash = sha256_hex(&decoded);
                    let decoded_len = saturating_u64(decoded.len());
                    if wants(wanted_entry, wanted_entries, &id) {
                        stored_payloads.insert(id.clone(), stored);
                        decoded_payloads.insert(id.clone(), decoded);
                    }
                    decoded_bytes = decoded_bytes.saturating_add(decoded_len);
                    decoded_streams = decoded_streams.saturating_add(1);
                    entries.push(InventoryEntry {
                        id,
                        path: Some(fact.path),
                        kind,
                        state: InventoryEntryState::Decoded,
                        source_range: None,
                        payload_range: None,
                        stored_size: fact.len,
                        decoded_size: Some(decoded_len),
                        compression: CompressionMethod::Zlib,
                        checksum: ChecksumStatus::NotPresent,
                        expected_crc32: None,
                        stored_sha256: Some(stored_hash),
                        decoded_sha256: Some(decoded_hash),
                        attributes,
                    });
                }
                Err(failure) => {
                    let declared_decoded_size = u32_le(&stored, 16).map(u64::from);
                    let (diagnostic, rejected) = zlb_diagnostic(&fact.path, &stored, failure);
                    diagnostics.push(diagnostic);
                    if rejected {
                        fatal = true;
                    } else {
                        malformed_stream = true;
                    }
                    if wants(wanted_entry, wanted_entries, &id) {
                        stored_payloads.insert(id.clone(), stored);
                    }
                    entries.push(InventoryEntry {
                        id,
                        path: Some(fact.path),
                        kind,
                        state: InventoryEntryState::Malformed,
                        source_range: None,
                        payload_range: None,
                        stored_size: fact.len,
                        decoded_size: declared_decoded_size,
                        compression: CompressionMethod::Zlib,
                        checksum: ChecksumStatus::NotPresent,
                        expected_crc32: None,
                        stored_sha256: Some(stored_hash),
                        decoded_sha256: None,
                        attributes,
                    });
                    if fatal {
                        break;
                    }
                }
            }
        } else {
            if fact.len
                > limits
                    .max_total_uncompressed_bytes
                    .saturating_sub(decoded_bytes)
            {
                fatal = true;
                diagnostics.push(
                    Diagnostic::new(
                        "limit.uncompressed_bytes",
                        DiagnosticSeverity::Error,
                        DiagnosticKind::Fatal,
                        "decoded CFB stream total exceeds the configured limit",
                    )
                    .at_offset(48)
                    .in_stream(fact.path.clone())
                    .with_detail("stream_bytes", fact.len.to_string())
                    .with_detail(
                        "remaining_bytes",
                        limits
                            .max_total_uncompressed_bytes
                            .saturating_sub(decoded_bytes)
                            .to_string(),
                    ),
                );
                break;
            }
            if stored.starts_with(ZLB_MAGIC) {
                diagnostics.push(
                    Diagnostic::new(
                        "ole2.unexpected_zlb_wrapper",
                        DiagnosticSeverity::Warning,
                        DiagnosticKind::Preserved,
                        "stream starts with a ZLB wrapper but its name does not opt into decoding",
                    )
                    .at_offset(48)
                    .in_stream(fact.path.clone()),
                );
            }
            if wants(wanted_entry, wanted_entries, &id) {
                stored_payloads.insert(id.clone(), stored.clone());
                decoded_payloads.insert(id.clone(), stored.clone());
            }
            decoded_bytes = decoded_bytes.saturating_add(fact.len);
            decoded_streams = decoded_streams.saturating_add(1);
            entries.push(InventoryEntry {
                id,
                path: Some(fact.path),
                kind,
                state: InventoryEntryState::Decoded,
                source_range: None,
                payload_range: None,
                stored_size: fact.len,
                decoded_size: Some(fact.len),
                compression: CompressionMethod::None,
                checksum: ChecksumStatus::NotPresent,
                expected_crc32: None,
                stored_sha256: Some(stored_hash.clone()),
                decoded_sha256: Some(stored_hash),
                attributes,
            });
        }
    }

    let status = if fatal {
        InventoryStatus::Rejected
    } else if malformed_stream {
        InventoryStatus::Partial
    } else {
        InventoryStatus::Complete
    };
    // A successful CFB traversal classifies all sectors as header, allocation,
    // directory, stream, free, or padding even when stream bytes are fragmented.
    accounted.add(0, data.len());
    finish(
        data,
        header,
        entries,
        diagnostics,
        stored_payloads,
        decoded_payloads,
        decoded_bytes,
        decoded_streams,
        status,
        &accounted,
    )
}

fn wants(
    wanted_entry: Option<&str>,
    wanted_entries: Option<&BTreeSet<String>>,
    entry_id: &str,
) -> bool {
    wanted_entry == Some(entry_id)
        || wanted_entries.is_some_and(|entries| entries.contains(entry_id))
}

#[allow(clippy::too_many_lines)]
fn preflight(data: &[u8], accounted: &mut RangeSet) -> Result<HeaderFacts, Diagnostic> {
    if data.len() < CFB_HEADER_LEN {
        accounted.add(0, data.len());
        return Err(Diagnostic::new(
            "ole2.truncated_header",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "CFB input ends before the 512-byte header is complete",
        )
        .at_offset(saturating_u64(data.len())));
    }
    accounted.add(0, CFB_HEADER_LEN);
    if data.get(28..30) != Some(&[0xfe, 0xff]) {
        return Err(Diagnostic::new(
            "ole2.invalid_byte_order",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "CFB header does not declare little-endian byte order",
        )
        .at_offset(28));
    }
    let major_version = u16_le(data, 26).unwrap_or(0);
    let expected_shift = match major_version {
        3 => 9,
        4 => 12,
        _ => {
            return Err(Diagnostic::new(
                "ole2.unsupported_version",
                DiagnosticSeverity::Error,
                DiagnosticKind::Unsupported,
                "CFB major version is not version 3 or 4",
            )
            .at_offset(26)
            .with_detail("major_version", major_version.to_string()));
        }
    };
    let sector_shift = u16_le(data, 30).unwrap_or(0);
    if sector_shift != expected_shift {
        return Err(Diagnostic::new(
            "ole2.invalid_sector_shift",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "CFB sector shift does not match its major version",
        )
        .at_offset(30)
        .with_detail("actual", sector_shift.to_string())
        .with_detail("expected", expected_shift.to_string()));
    }
    if u16_le(data, 32) != Some(6) {
        return Err(Diagnostic::new(
            "ole2.invalid_mini_sector_shift",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "CFB mini-sector shift must be six",
        )
        .at_offset(32));
    }
    let sector_size = 1_usize << usize::from(sector_shift);
    if data.len() < sector_size {
        return Err(Diagnostic::new(
            "ole2.truncated_header_sector",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "CFB input ends inside its initial header sector",
        )
        .at_offset(saturating_u64(data.len())));
    }
    accounted.add(0, sector_size);
    let physical_sectors = saturating_u64(data.len() / sector_size).saturating_sub(1);
    for (offset, label) in [
        (44, "fat_sectors"),
        (64, "mini_fat_sectors"),
        (72, "difat_sectors"),
    ] {
        let declared = u64::from(u32_le(data, offset).unwrap_or(u32::MAX));
        if declared > physical_sectors {
            return Err(Diagnostic::new(
                "ole2.impossible_sector_count",
                DiagnosticSeverity::Error,
                DiagnosticKind::Malformed,
                "CFB header declares more allocation sectors than the file can contain",
            )
            .at_offset(offset as u64)
            .with_detail("field", label)
            .with_detail("declared", declared.to_string())
            .with_detail("physical_sectors", physical_sectors.to_string()));
        }
    }
    if major_version == 4 {
        let declared = u64::from(u32_le(data, 40).unwrap_or(u32::MAX));
        if declared > physical_sectors {
            return Err(Diagnostic::new(
                "ole2.impossible_sector_count",
                DiagnosticSeverity::Error,
                DiagnosticKind::Malformed,
                "CFB header declares more directory sectors than the file can contain",
            )
            .at_offset(40)
            .with_detail("field", "directory_sectors")
            .with_detail("declared", declared.to_string())
            .with_detail("physical_sectors", physical_sectors.to_string()));
        }
    }
    Ok(HeaderFacts {
        major_version,
        sector_size,
        physical_sectors,
    })
}

fn read_stream(
    compound: &mut cfb::CompoundFile<Cursor<&[u8]>>,
    path: &str,
    expected_len: u64,
) -> std::io::Result<Vec<u8>> {
    let stream = compound.open_stream(path)?;
    let mut output = Vec::new();
    stream
        .take(expected_len.saturating_add(1))
        .read_to_end(&mut output)?;
    if saturating_u64(output.len()) != expected_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "CFB stream length mismatch",
        ));
    }
    Ok(output)
}

fn decode_zlb(
    stored: &[u8],
    already_decoded: u64,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, ZlbFailure> {
    if stored.len() < ZLB_HEADER_LEN || !stored.starts_with(ZLB_MAGIC) {
        return Err(ZlbFailure::InvalidWrapper);
    }
    let expected_size = u64::from(u32_le(stored, 16).ok_or(ZlbFailure::InvalidWrapper)?);
    let member_size = u64::from(u32_le(stored, 20).ok_or(ZlbFailure::InvalidWrapper)?);
    if stored.len() == ZLB_HEADER_LEN && expected_size == 0 && member_size == 0 {
        return Ok(Vec::new());
    }
    let member_size_usize = usize_from_u64(member_size).ok_or(ZlbFailure::NumericRange)?;
    let member_end =
        checked_end(ZLB_HEADER_LEN, &[member_size_usize]).ok_or(ZlbFailure::NumericRange)?;
    let wrapper_end = member_end
        .checked_add(ZLB_TRAILER_LEN)
        .ok_or(ZlbFailure::NumericRange)?;
    if wrapper_end != stored.len() {
        return Err(ZlbFailure::InvalidWrapper);
    }
    inflate_zlib_bounded(
        &stored[ZLB_HEADER_LEN..member_end],
        expected_size,
        already_decoded,
        limits,
    )
    .map_err(|failure| ZlbFailure::Inflate {
        failure,
        expected_size,
        member_size,
    })
}

#[derive(Clone, Copy)]
enum ZlbFailure {
    InvalidWrapper,
    NumericRange,
    Inflate {
        failure: InflateFailure,
        expected_size: u64,
        member_size: u64,
    },
}

fn zlb_diagnostic(path: &str, stored: &[u8], failure: ZlbFailure) -> (Diagnostic, bool) {
    match failure {
        ZlbFailure::InvalidWrapper => (
            Diagnostic::new(
                "ole2.invalid_zlb_wrapper",
                DiagnosticSeverity::Error,
                DiagnosticKind::Malformed,
                "__ZLB stream does not contain one complete bounded wrapper",
            )
            .at_offset(48)
            .in_stream(path)
            .with_detail("stored_bytes", stored.len().to_string()),
            false,
        ),
        ZlbFailure::NumericRange => (
            Diagnostic::new(
                "limit.numeric_range",
                DiagnosticSeverity::Error,
                DiagnosticKind::Fatal,
                "ZLB member range cannot be represented safely",
            )
            .at_offset(48)
            .in_stream(path),
            true,
        ),
        ZlbFailure::Inflate {
            failure,
            expected_size,
            member_size,
        } => {
            let rejected = matches!(
                failure,
                InflateFailure::DeclaredSizeLimit | InflateFailure::CompressionRatio
            );
            (
                inflate_diagnostic(failure, 48, expected_size, member_size).in_stream(path),
                rejected,
            )
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn finish(
    data: &[u8],
    header: HeaderFacts,
    entries: Vec<InventoryEntry>,
    diagnostics: Vec<Diagnostic>,
    stored: BTreeMap<String, Vec<u8>>,
    decoded: BTreeMap<String, Vec<u8>>,
    decoded_bytes: u64,
    decoded_streams: u64,
    status: InventoryStatus,
    accounted: &RangeSet,
) -> ScanArtifacts {
    let uninterpreted_ranges = accounted.complement(data.len());
    let streams_total = entries
        .iter()
        .filter(|entry| entry.kind == InventoryEntryKind::Stream)
        .count();
    let result = InventoryResult {
        status,
        inventory: Some(ContainerInventory {
            envelope: Envelope::Ole2Cfb,
            format_version: Some(u64::from(header.major_version)),
            entries,
            attributes: BTreeMap::from([
                ("sector_size".to_owned(), header.sector_size.to_string()),
                (
                    "physical_sectors".to_owned(),
                    header.physical_sectors.to_string(),
                ),
                (
                    "byte_accounting".to_owned(),
                    "cfb_structural_allocation".to_owned(),
                ),
            ]),
        }),
        diagnostics,
        coverage: CoverageReport {
            total_bytes: saturating_u64(data.len()),
            inspected_bytes: if status == InventoryStatus::Complete
                || status == InventoryStatus::Partial
            {
                saturating_u64(data.len())
            } else {
                CFB_HEADER_LEN.min(data.len()) as u64
            },
            decoded_bytes,
            uninterpreted_bytes: range_sum(&uninterpreted_ranges),
            streams_total: saturating_u64(streams_total),
            streams_decoded: decoded_streams,
        },
        uninterpreted_ranges,
    };
    ScanArtifacts {
        result,
        stored,
        decoded,
    }
}

fn failed_scan(
    data: &[u8],
    diagnostic: Diagnostic,
    accounted: &RangeSet,
    status: InventoryStatus,
) -> ScanArtifacts {
    let header = HeaderFacts {
        major_version: u16_le(data, 26).unwrap_or(0),
        sector_size: 0,
        physical_sectors: 0,
    };
    finish(
        data,
        header,
        Vec::new(),
        vec![diagnostic],
        BTreeMap::new(),
        BTreeMap::new(),
        0,
        0,
        status,
        accounted,
    )
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use cfb::Version;
    use sldkit_core::{InventoryEntryKind, InventoryStatus, ResourceLimits};

    use super::{ZLB_HEADER_LEN, ZLB_MAGIC, decode_zlb, scan};

    fn fixture(version: Version) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let cursor = Cursor::new(Vec::new());
        let mut compound = cfb::CompoundFile::create_with_version(version, cursor)?;
        compound.create_storage("/Contents")?;
        {
            let mut stream = compound.create_stream("/Contents/Test")?;
            stream.write_all(b"payload")?;
        }
        compound.flush()?;
        Ok(compound.into_inner().into_inner())
    }

    #[test]
    fn walks_v3_and_v4_compound_files_deterministically() -> Result<(), Box<dyn std::error::Error>>
    {
        for version in [Version::V3, Version::V4] {
            let data = fixture(version)?;
            let first = scan(&data, &ResourceLimits::service(), None);
            let second = scan(&data, &ResourceLimits::service(), None);
            assert_eq!(first.result, second.result);
            assert_eq!(first.result.status, InventoryStatus::Complete);
            let inventory = first.result.inventory.as_ref();
            assert!(inventory.is_some());
            if let Some(inventory) = inventory {
                assert!(
                    inventory
                        .entries
                        .iter()
                        .any(|entry| entry.kind == InventoryEntryKind::Stream)
                );
            }
        }
        Ok(())
    }

    #[test]
    fn rejects_impossible_fat_count_in_preflight() -> Result<(), Box<dyn std::error::Error>> {
        let mut data = fixture(Version::V3)?;
        data[44..48].copy_from_slice(&u32::MAX.to_le_bytes());
        let scanned = scan(&data, &ResourceLimits::service(), None);
        assert_eq!(scanned.result.status, InventoryStatus::Malformed);
        assert_eq!(scanned.result.diagnostics[0].offset, Some(44));
        Ok(())
    }

    #[test]
    fn accepts_observed_empty_zlb_header_without_a_trailer() {
        let mut wrapper = Vec::from(ZLB_MAGIC.as_slice());
        wrapper.resize(ZLB_HEADER_LEN, 0);
        let decoded = decode_zlb(&wrapper, 0, &ResourceLimits::service());
        assert!(decoded.is_ok());
        if let Ok(decoded) = decoded {
            assert!(decoded.is_empty());
        }
    }
}
