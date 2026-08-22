//! Bounded, content-based envelope probing, inventory, and extraction.

mod common;
mod modern;
mod ole2;
mod zip;

use std::collections::{BTreeMap, BTreeSet};

use sldkit_core::{
    ByteRange, CoverageReport, Diagnostic, DiagnosticKind, DiagnosticSeverity, Envelope,
    ExtractionMode, InventoryResult, InventoryStatus, ProbeConfidence, ProbeEvidence, ProbeResult,
    ProbeStatus, ResourceLimits, StreamExtraction,
};

use common::ScanArtifacts;

const OLE2_CFB_SIGNATURE: &[u8] = &[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
const ZIP_LOCAL_SIGNATURE: &[u8] = b"PK\x03\x04";
const ZIP_EMPTY_SIGNATURE: &[u8] = b"PK\x05\x06";
const ZIP_SPANNED_SIGNATURE: &[u8] = b"PK\x07\x08";
const PROBE_WINDOW: usize = 64;

/// Selected validated decoded streams plus the inventory produced by the same scan.
#[derive(Debug)]
pub struct DecodedStreams {
    pub result: InventoryResult,
    pub streams: BTreeMap<String, Vec<u8>>,
}

/// Inspect bounded leading bytes and classify a candidate container envelope.
#[must_use]
pub fn probe_bytes(data: &[u8], limits: &ResourceLimits) -> ProbeResult {
    let total_bytes = u64::try_from(data.len()).unwrap_or(u64::MAX);
    let inspected_len = data.len().min(PROBE_WINDOW);
    let coverage = coverage(total_bytes, inspected_len);

    if let Some(result) = initial_rejection(data, limits, coverage) {
        return result;
    }

    if data.starts_with(OLE2_CFB_SIGNATURE) {
        return recognized_result(
            Envelope::Ole2Cfb,
            ProbeConfidence::High,
            coverage,
            "signature.ole2_cfb",
            0,
            OLE2_CFB_SIGNATURE.len(),
            "OLE2 Compound File Binary signature",
        );
    }

    if let Some(signature) = zip_signature(data) {
        return recognized_result(
            Envelope::ZipOpc,
            ProbeConfidence::High,
            coverage,
            "signature.zip",
            0,
            signature.len(),
            "ZIP signature; OPC semantics are not yet decoded",
        );
    }

    let probe_window = &data[..inspected_len];
    if let Some(offset) = probe_window
        .windows(modern::MARKER.len())
        .position(|window| window == modern::MARKER)
    {
        return recognized_result(
            Envelope::ModernChunk,
            ProbeConfidence::Medium,
            coverage,
            "marker.modern_chunk_candidate",
            offset,
            modern::MARKER.len(),
            "candidate modern SolidWorks chunk marker",
        );
    }

    if let Some(expected) = truncated_signature_name(data) {
        return malformed_result(
            coverage,
            "input.truncated_signature",
            "input ends inside a recognized container signature",
            Some(expected),
        );
    }

    ProbeResult {
        status: ProbeStatus::Unrecognized,
        envelope: Envelope::Unknown,
        confidence: ProbeConfidence::None,
        evidence: Vec::new(),
        diagnostics: vec![Diagnostic::new(
            "format.unrecognized",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Unsupported,
            "input does not match a recognized SolidWorks container candidate",
        )],
        coverage,
    }
}

/// Walk the recognized outer container and return a deterministic inventory.
#[must_use]
pub fn inspect_bytes(data: &[u8], limits: &ResourceLimits) -> InventoryResult {
    scan_bytes(data, limits, None).result
}

/// Extract one inventory entry representation by its stable ID.
///
/// The container is rescanned under the same limits so callers cannot bypass
/// validation with an ID obtained under a different input or policy.
#[must_use]
pub fn extract_bytes(
    data: &[u8],
    entry_id: &str,
    mode: ExtractionMode,
    limits: &ResourceLimits,
) -> StreamExtraction {
    scan_bytes(data, limits, Some(entry_id)).extraction(entry_id, mode)
}

/// Re-scan once and retain only the decoded entry IDs requested by a semantic layer.
///
/// IDs that are absent, malformed, or have no decoded representation are omitted;
/// callers must use `result.inventory` and diagnostics to distinguish those cases.
#[must_use]
pub fn decode_selected_bytes(
    data: &[u8],
    entry_ids: &BTreeSet<String>,
    limits: &ResourceLimits,
) -> DecodedStreams {
    let artifacts = scan_selected_bytes(data, limits, entry_ids);
    DecodedStreams {
        result: artifacts.result,
        streams: artifacts.decoded,
    }
}

fn scan_bytes(data: &[u8], limits: &ResourceLimits, wanted_entry: Option<&str>) -> ScanArtifacts {
    let probe = probe_bytes(data, limits);
    match probe.status {
        ProbeStatus::Recognized => match probe.envelope {
            Envelope::ModernChunk => modern::scan(data, limits, wanted_entry),
            Envelope::Ole2Cfb => ole2::scan(data, limits, wanted_entry),
            Envelope::ZipOpc => zip::scan(data, limits, wanted_entry),
            Envelope::Unknown => scan_from_probe(probe, InventoryStatus::Unsupported),
        },
        ProbeStatus::Rejected => scan_from_probe(probe, InventoryStatus::Rejected),
        ProbeStatus::Malformed => scan_from_probe(probe, InventoryStatus::Malformed),
        ProbeStatus::Unrecognized => scan_from_probe(probe, InventoryStatus::Unsupported),
    }
}

fn scan_selected_bytes(
    data: &[u8],
    limits: &ResourceLimits,
    wanted_entries: &BTreeSet<String>,
) -> ScanArtifacts {
    let probe = probe_bytes(data, limits);
    match probe.status {
        ProbeStatus::Recognized => match probe.envelope {
            Envelope::ModernChunk => modern::scan_selected(data, limits, wanted_entries),
            Envelope::Ole2Cfb => ole2::scan_selected(data, limits, wanted_entries),
            Envelope::ZipOpc => zip::scan_selected(data, limits, wanted_entries),
            Envelope::Unknown => scan_from_probe(probe, InventoryStatus::Unsupported),
        },
        ProbeStatus::Rejected => scan_from_probe(probe, InventoryStatus::Rejected),
        ProbeStatus::Malformed => scan_from_probe(probe, InventoryStatus::Malformed),
        ProbeStatus::Unrecognized => scan_from_probe(probe, InventoryStatus::Unsupported),
    }
}

fn scan_from_probe(probe: ProbeResult, status: InventoryStatus) -> ScanArtifacts {
    let uninterpreted_ranges = (probe.coverage.total_bytes > 0)
        .then(|| ByteRange::new(0, probe.coverage.total_bytes))
        .into_iter()
        .collect();
    ScanArtifacts {
        result: InventoryResult {
            status,
            inventory: None,
            diagnostics: probe.diagnostics,
            coverage: probe.coverage,
            uninterpreted_ranges,
        },
        stored: std::collections::BTreeMap::new(),
        decoded: std::collections::BTreeMap::new(),
    }
}

fn coverage(total_bytes: u64, inspected_len: usize) -> CoverageReport {
    CoverageReport {
        total_bytes,
        inspected_bytes: u64::try_from(inspected_len).unwrap_or(u64::MAX),
        decoded_bytes: 0,
        uninterpreted_bytes: total_bytes,
        streams_total: 0,
        streams_decoded: 0,
    }
}

fn initial_rejection(
    data: &[u8],
    limits: &ResourceLimits,
    coverage: CoverageReport,
) -> Option<ProbeResult> {
    let total_bytes = coverage.total_bytes;
    if total_bytes > limits.max_file_size {
        return Some(ProbeResult {
            status: ProbeStatus::Rejected,
            envelope: Envelope::Unknown,
            confidence: ProbeConfidence::None,
            evidence: Vec::new(),
            diagnostics: vec![
                Diagnostic::new(
                    "limit.file_size",
                    DiagnosticSeverity::Error,
                    DiagnosticKind::Fatal,
                    "input exceeds the configured file-size limit",
                )
                .with_detail("actual_bytes", total_bytes.to_string())
                .with_detail("limit_bytes", limits.max_file_size.to_string()),
            ],
            coverage,
        });
    }

    data.is_empty().then(|| {
        malformed_result(
            coverage,
            "input.empty",
            "empty input cannot contain a SolidWorks document",
            None,
        )
    })
}

fn zip_signature(data: &[u8]) -> Option<&'static [u8]> {
    [
        ZIP_LOCAL_SIGNATURE,
        ZIP_EMPTY_SIGNATURE,
        ZIP_SPANNED_SIGNATURE,
    ]
    .into_iter()
    .find(|signature| data.starts_with(signature))
}

fn recognized_result(
    envelope: Envelope,
    confidence: ProbeConfidence,
    coverage: CoverageReport,
    code: &str,
    offset: usize,
    length: usize,
    description: &str,
) -> ProbeResult {
    ProbeResult {
        status: ProbeStatus::Recognized,
        envelope,
        confidence,
        evidence: vec![ProbeEvidence {
            code: code.to_owned(),
            offset: u64::try_from(offset).unwrap_or(u64::MAX),
            length: u64::try_from(length).unwrap_or(u64::MAX),
            description: description.to_owned(),
        }],
        diagnostics: Vec::new(),
        coverage,
    }
}

fn malformed_result(
    coverage: CoverageReport,
    code: &str,
    message: &str,
    expected: Option<&str>,
) -> ProbeResult {
    let mut diagnostic = Diagnostic::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticKind::Malformed,
        message,
    )
    .at_offset(0);
    if let Some(value) = expected {
        diagnostic = diagnostic.with_detail("expected", value);
    }

    ProbeResult {
        status: ProbeStatus::Malformed,
        envelope: Envelope::Unknown,
        confidence: ProbeConfidence::None,
        evidence: Vec::new(),
        diagnostics: vec![diagnostic],
        coverage,
    }
}

fn truncated_signature_name(data: &[u8]) -> Option<&'static str> {
    if is_nonempty_prefix(data, OLE2_CFB_SIGNATURE) {
        return Some("ole2_cfb");
    }
    if [
        ZIP_LOCAL_SIGNATURE,
        ZIP_EMPTY_SIGNATURE,
        ZIP_SPANNED_SIGNATURE,
    ]
    .into_iter()
    .any(|signature| is_nonempty_prefix(data, signature))
    {
        return Some("zip");
    }
    if is_nonempty_prefix(data, modern::MARKER) {
        return Some("modern_chunk_candidate");
    }
    None
}

fn is_nonempty_prefix(data: &[u8], signature: &[u8]) -> bool {
    !data.is_empty() && data.len() < signature.len() && signature.starts_with(data)
}

#[cfg(test)]
mod tests {
    use sldkit_core::{DiagnosticKind, Envelope, ProbeConfidence, ProbeStatus, ResourceLimits};

    use super::{OLE2_CFB_SIGNATURE, probe_bytes};

    #[test]
    fn recognizes_ole2_from_content() {
        let result = probe_bytes(OLE2_CFB_SIGNATURE, &ResourceLimits::desktop());
        assert_eq!(result.status, ProbeStatus::Recognized);
        assert_eq!(result.envelope, Envelope::Ole2Cfb);
        assert_eq!(result.confidence, ProbeConfidence::High);
    }

    #[test]
    fn recognizes_zip_from_content() {
        let result = probe_bytes(b"PK\x03\x04payload", &ResourceLimits::desktop());
        assert_eq!(result.status, ProbeStatus::Recognized);
        assert_eq!(result.envelope, Envelope::ZipOpc);
    }

    #[test]
    fn recognizes_modern_marker_in_bounded_header() {
        let mut data = vec![0xaa; 12];
        data.extend_from_slice(super::modern::MARKER);
        let result = probe_bytes(&data, &ResourceLimits::desktop());
        assert_eq!(result.status, ProbeStatus::Recognized);
        assert_eq!(result.envelope, Envelope::ModernChunk);
        assert_eq!(result.confidence, ProbeConfidence::Medium);
        assert_eq!(result.evidence[0].offset, 12);
    }

    #[test]
    fn distinguishes_malformed_prefix_from_unsupported_input() {
        let malformed = probe_bytes(&OLE2_CFB_SIGNATURE[..4], &ResourceLimits::desktop());
        let unsupported = probe_bytes(b"not a container", &ResourceLimits::desktop());

        assert_eq!(malformed.status, ProbeStatus::Malformed);
        assert_eq!(malformed.diagnostics[0].kind, DiagnosticKind::Malformed);
        assert_eq!(unsupported.status, ProbeStatus::Unrecognized);
        assert_eq!(unsupported.diagnostics[0].kind, DiagnosticKind::Unsupported);
    }

    #[test]
    fn rejects_input_over_configured_limit() {
        let mut limits = ResourceLimits::service();
        limits.max_file_size = 3;
        let result = probe_bytes(b"four", &limits);
        assert_eq!(result.status, ProbeStatus::Rejected);
        assert_eq!(result.diagnostics[0].code, "limit.file_size");
        assert_eq!(result.diagnostics[0].kind, DiagnosticKind::Fatal);
    }
}
