use std::{collections::BTreeMap, io::Read};

use flate2::read::{DeflateDecoder, ZlibDecoder};
use sha2::{Digest, Sha256};
use sldkit_core::{
    ByteRange, Diagnostic, DiagnosticKind, DiagnosticSeverity, ExtractionMode, ExtractionResult,
    ExtractionStatus, InventoryResult, ResourceLimits, StreamExtraction,
};

#[derive(Debug)]
pub(crate) struct ScanArtifacts {
    pub result: InventoryResult,
    pub stored: BTreeMap<String, Vec<u8>>,
    pub decoded: BTreeMap<String, Vec<u8>>,
}

impl ScanArtifacts {
    pub(crate) fn extraction(self, entry_id: &str, mode: ExtractionMode) -> StreamExtraction {
        let entry = self
            .result
            .inventory
            .as_ref()
            .and_then(|inventory| inventory.entries.iter().find(|entry| entry.id == entry_id))
            .cloned();

        let Some(entry) = entry else {
            let (status, diagnostics) = match self.result.status {
                sldkit_core::InventoryStatus::Rejected => {
                    (ExtractionStatus::Rejected, self.result.diagnostics)
                }
                sldkit_core::InventoryStatus::Malformed => {
                    (ExtractionStatus::Malformed, self.result.diagnostics)
                }
                sldkit_core::InventoryStatus::Unsupported => {
                    (ExtractionStatus::Unavailable, self.result.diagnostics)
                }
                sldkit_core::InventoryStatus::Complete | sldkit_core::InventoryStatus::Partial => (
                    ExtractionStatus::NotFound,
                    vec![
                        Diagnostic::new(
                            "extract.entry_not_found",
                            DiagnosticSeverity::Error,
                            DiagnosticKind::Unresolved,
                            "inventory entry ID was not found",
                        )
                        .with_detail("entry_id", entry_id),
                    ],
                ),
            };
            return StreamExtraction {
                result: ExtractionResult {
                    status,
                    mode,
                    entry: None,
                    byte_len: None,
                    sha256: None,
                    diagnostics,
                },
                data: None,
            };
        };

        let payload = match mode {
            ExtractionMode::Stored => self.stored.get(entry_id),
            ExtractionMode::Decoded => self.decoded.get(entry_id),
        };
        let Some(payload) = payload else {
            return StreamExtraction {
                result: ExtractionResult {
                    status: ExtractionStatus::Unavailable,
                    mode,
                    entry: Some(entry),
                    byte_len: None,
                    sha256: None,
                    diagnostics: vec![
                        Diagnostic::new(
                            "extract.mode_unavailable",
                            DiagnosticSeverity::Error,
                            DiagnosticKind::Unsupported,
                            "requested representation is unavailable for this inventory entry",
                        )
                        .with_detail("entry_id", entry_id)
                        .with_detail(
                            "mode",
                            match mode {
                                ExtractionMode::Stored => "stored",
                                ExtractionMode::Decoded => "decoded",
                            },
                        ),
                    ],
                },
                data: None,
            };
        };

        let data = payload.clone();
        StreamExtraction {
            result: ExtractionResult {
                status: ExtractionStatus::Extracted,
                mode,
                entry: Some(entry),
                byte_len: Some(saturating_u64(data.len())),
                sha256: Some(sha256_hex(&data)),
                diagnostics: Vec::new(),
            },
            data: Some(data),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InflateFailure {
    DeclaredSizeLimit,
    CompressionRatio,
    Io,
    SizeMismatch,
}

pub(crate) fn inflate_raw_bounded(
    compressed: &[u8],
    expected_size: u64,
    already_decoded: u64,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, InflateFailure> {
    inflate_bounded(
        DeflateDecoder::new(compressed),
        compressed.len(),
        expected_size,
        already_decoded,
        limits,
    )
}

pub(crate) fn inflate_zlib_bounded(
    compressed: &[u8],
    expected_size: u64,
    already_decoded: u64,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, InflateFailure> {
    inflate_bounded(
        ZlibDecoder::new(compressed),
        compressed.len(),
        expected_size,
        already_decoded,
        limits,
    )
}

fn inflate_bounded(
    decoder: impl Read,
    compressed_size: usize,
    expected_size: u64,
    already_decoded: u64,
    limits: &ResourceLimits,
) -> Result<Vec<u8>, InflateFailure> {
    if expected_size
        > limits
            .max_total_uncompressed_bytes
            .saturating_sub(already_decoded)
    {
        return Err(InflateFailure::DeclaredSizeLimit);
    }
    let compressed_size = saturating_u64(compressed_size);
    if expected_size > 0
        && (compressed_size == 0
            || expected_size > compressed_size.saturating_mul(limits.max_compression_ratio))
    {
        return Err(InflateFailure::CompressionRatio);
    }

    let read_cap = expected_size.saturating_add(1);
    let mut output = Vec::new();
    decoder
        .take(read_cap)
        .read_to_end(&mut output)
        .map_err(|_| InflateFailure::Io)?;
    if saturating_u64(output.len()) != expected_size {
        return Err(InflateFailure::SizeMismatch);
    }
    Ok(output)
}

pub(crate) fn inflate_diagnostic(
    failure: InflateFailure,
    offset: u64,
    expected_size: u64,
    compressed_size: u64,
) -> Diagnostic {
    let (code, kind, message) = match failure {
        InflateFailure::DeclaredSizeLimit => (
            "limit.uncompressed_bytes",
            DiagnosticKind::Fatal,
            "declared decompressed payload exceeds the configured total limit",
        ),
        InflateFailure::CompressionRatio => (
            "limit.compression_ratio",
            DiagnosticKind::Fatal,
            "declared compression ratio exceeds the configured limit",
        ),
        InflateFailure::Io => (
            "compression.invalid_stream",
            DiagnosticKind::Malformed,
            "compressed payload is not a valid stream",
        ),
        InflateFailure::SizeMismatch => (
            "compression.size_mismatch",
            DiagnosticKind::Malformed,
            "decompressed payload length differs from its declared length",
        ),
    };
    Diagnostic::new(code, DiagnosticSeverity::Error, kind, message)
        .at_offset(offset)
        .with_detail("compressed_bytes", compressed_size.to_string())
        .with_detail("declared_uncompressed_bytes", expected_size.to_string())
}

#[must_use]
pub(crate) fn sha256_hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(data);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[derive(Debug, Default)]
pub(crate) struct RangeSet {
    ranges: Vec<(usize, usize)>,
}

impl RangeSet {
    pub(crate) fn add(&mut self, start: usize, end: usize) {
        if start < end {
            self.ranges.push((start, end));
        }
    }

    pub(crate) fn complement(&self, total: usize) -> Vec<ByteRange> {
        let mut ranges = self.ranges.clone();
        ranges.sort_unstable();
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (start, end) in ranges {
            let end = end.min(total);
            if start >= end || start >= total {
                continue;
            }
            if let Some(last) = merged.last_mut()
                && start <= last.1
            {
                last.1 = last.1.max(end);
                continue;
            }
            merged.push((start, end));
        }

        let mut output = Vec::new();
        let mut cursor = 0;
        for (start, end) in merged {
            if cursor < start {
                output.push(ByteRange::new(
                    saturating_u64(cursor),
                    saturating_u64(start - cursor),
                ));
            }
            cursor = cursor.max(end);
        }
        if cursor < total {
            output.push(ByteRange::new(
                saturating_u64(cursor),
                saturating_u64(total - cursor),
            ));
        }
        output
    }
}

pub(crate) fn range_sum(ranges: &[ByteRange]) -> u64 {
    ranges
        .iter()
        .fold(0_u64, |total, range| total.saturating_add(range.length))
}

pub(crate) fn saturating_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

pub(crate) fn u16_le(data: &[u8], offset: usize) -> Option<u16> {
    let bytes: [u8; 2] = data.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(bytes))
}

pub(crate) fn u32_le(data: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

pub(crate) fn u32_be(data: &[u8], offset: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_be_bytes(bytes))
}

pub(crate) fn usize_from_u64(value: u64) -> Option<usize> {
    usize::try_from(value).ok()
}

pub(crate) fn checked_end(start: usize, lengths: &[usize]) -> Option<usize> {
    lengths
        .iter()
        .try_fold(start, |position, length| position.checked_add(*length))
}
