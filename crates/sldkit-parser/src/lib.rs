//! High-level path and byte entry points for `SolidWorks` parsing.

mod geometry;
mod legacy_semantics;
mod modern_semantics;
mod project;

pub use project::{ProjectScanOptions, WindowsPrefixMapping, scan_project_path};

use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use sldkit_container::{
    extract_bytes as extract_container_bytes, inspect_bytes as inspect_container_bytes,
    probe_bytes as probe_container_bytes,
};
use sldkit_core::{
    BinaryResource, CoverageReport, Diagnostic, DiagnosticKind, DiagnosticSeverity, DocumentKind,
    Envelope, ExtractionMode, ExtractionResult, ExtractionStatus, GeometryResult, GeometryStatus,
    InventoryResult, InventoryStatus, ParseResult, ParseStatus, ProbeConfidence, ProbeResult,
    ProbeStatus, ResourceLimits, SourceDocument, SourceInfo, SourceInputKind, SourceValue,
    StreamExtraction, ValueOrigin,
};

/// Probe bytes without relying on a filename extension.
#[must_use]
pub fn probe_bytes(data: &[u8], limits: &ResourceLimits) -> ProbeResult {
    probe_container_bytes(data, limits)
}

/// Read a path through the configured file-size bound, then probe its contents.
#[must_use]
pub fn probe_path(path: impl AsRef<Path>, limits: &ResourceLimits) -> ProbeResult {
    match read_path_bounded(path.as_ref(), limits) {
        Ok(data) => probe_bytes(&data, limits),
        Err(failure) => rejected_probe(*failure),
    }
}

/// Walk a byte input's outer container without interpreting document semantics.
#[must_use]
pub fn inspect_bytes(
    data: &[u8],
    filename: Option<&str>,
    limits: &ResourceLimits,
) -> InventoryResult {
    let mut result = inspect_container_bytes(data, limits);
    append_extension_diagnostic(
        &mut result.diagnostics,
        filename,
        result.inventory.is_some(),
    );
    result
}

/// Read a path under the configured bound and return its container inventory.
#[must_use]
pub fn inspect_path(path: impl AsRef<Path>, limits: &ResourceLimits) -> InventoryResult {
    let path = path.as_ref();
    let label = path.to_string_lossy().into_owned();
    match read_path_bounded(path, limits) {
        Ok(data) => inspect_bytes(&data, Some(&label), limits),
        Err(failure) => rejected_inventory(*failure),
    }
}

/// Extract a stored or decoded representation for one inventory entry ID.
#[must_use]
pub fn extract_bytes(
    data: &[u8],
    entry_id: &str,
    mode: ExtractionMode,
    limits: &ResourceLimits,
) -> StreamExtraction {
    extract_container_bytes(data, entry_id, mode, limits)
}

/// Read a bounded path and extract one entry without writing to the filesystem.
#[must_use]
pub fn extract_path(
    path: impl AsRef<Path>,
    entry_id: &str,
    mode: ExtractionMode,
    limits: &ResourceLimits,
) -> StreamExtraction {
    let path = path.as_ref();
    match read_path_bounded(path, limits) {
        Ok(data) => extract_bytes(&data, entry_id, mode, limits),
        Err(failure) => {
            let failure = *failure;
            StreamExtraction {
                result: ExtractionResult {
                    status: ExtractionStatus::Rejected,
                    mode,
                    entry: None,
                    byte_len: None,
                    sha256: None,
                    diagnostics: vec![failure.diagnostic],
                },
                data: None,
            }
        }
    }
}

/// Extract the exact byte range described by a parser-produced binary resource.
///
/// The containing entry is decoded under the supplied limits, then its path,
/// range, and SHA-256 digest are revalidated before any bytes are returned.
#[must_use]
pub fn extract_resource_bytes(
    data: &[u8],
    resource: &BinaryResource,
    limits: &ResourceLimits,
) -> StreamExtraction {
    let extraction = extract_bytes(data, &resource.entry_id, ExtractionMode::Decoded, limits);
    if extraction.result.status != ExtractionStatus::Extracted {
        return extraction;
    }

    let actual_path = extraction
        .result
        .entry
        .as_ref()
        .and_then(|entry| entry.path.as_deref());
    if actual_path != Some(resource.stream_path.as_str()) {
        let diagnostic = Diagnostic::new(
            "extract.resource_stream_mismatch",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "binary resource stream path does not match the extracted inventory entry",
        )
        .in_stream(resource.stream_path.clone())
        .with_detail("entry_id", resource.entry_id.clone())
        .with_detail("expected_path", resource.stream_path.clone())
        .with_detail("actual_path", actual_path.unwrap_or("<none>"));
        return malformed_resource_extraction(extraction, diagnostic);
    }

    let Some(decoded) = extraction.data.as_deref() else {
        let diagnostic = Diagnostic::new(
            "extract.resource_payload_missing",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "decoded entry was reported as extracted without a payload",
        )
        .in_stream(resource.stream_path.clone())
        .with_detail("entry_id", resource.entry_id.clone());
        return malformed_resource_extraction(extraction, diagnostic);
    };

    let end = resource
        .decoded_offset
        .checked_add(resource.byte_len)
        .filter(|end| *end <= u64::try_from(decoded.len()).unwrap_or(u64::MAX));
    let range = end.and_then(|end| {
        Some(usize::try_from(resource.decoded_offset).ok()?..usize::try_from(end).ok()?)
    });
    let Some(payload) = range.and_then(|range| decoded.get(range)) else {
        let diagnostic = Diagnostic::new(
            "extract.resource_range_invalid",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "binary resource range is outside the decoded inventory entry",
        )
        .at_offset(resource.decoded_offset)
        .in_stream(resource.stream_path.clone())
        .with_detail("entry_id", resource.entry_id.clone())
        .with_detail("resource_byte_len", resource.byte_len.to_string())
        .with_detail("decoded_byte_len", decoded.len().to_string());
        return malformed_resource_extraction(extraction, diagnostic);
    };

    let actual_sha256 = sha256_hex(payload);
    if actual_sha256 != resource.sha256 {
        let diagnostic = Diagnostic::new(
            "extract.resource_sha256_mismatch",
            DiagnosticSeverity::Error,
            DiagnosticKind::Malformed,
            "binary resource SHA-256 does not match the parser-produced descriptor",
        )
        .at_offset(resource.decoded_offset)
        .in_stream(resource.stream_path.clone())
        .with_detail("entry_id", resource.entry_id.clone())
        .with_detail("expected_sha256", resource.sha256.clone())
        .with_detail("actual_sha256", actual_sha256);
        return malformed_resource_extraction(extraction, diagnostic);
    }

    let data = payload.to_vec();
    let mut result = extraction.result;
    result.byte_len = Some(resource.byte_len);
    result.sha256 = Some(resource.sha256.clone());
    StreamExtraction {
        result,
        data: Some(data),
    }
}

/// Read a bounded path and extract one exact parser-produced binary resource.
#[must_use]
pub fn extract_resource_path(
    path: impl AsRef<Path>,
    resource: &BinaryResource,
    limits: &ResourceLimits,
) -> StreamExtraction {
    match read_path_bounded(path.as_ref(), limits) {
        Ok(data) => extract_resource_bytes(&data, resource, limits),
        Err(failure) => StreamExtraction {
            result: ExtractionResult {
                status: ExtractionStatus::Rejected,
                mode: ExtractionMode::Decoded,
                entry: None,
                byte_len: None,
                sha256: None,
                diagnostics: vec![failure.diagnostic],
            },
            data: None,
        },
    }
}

fn malformed_resource_extraction(
    extraction: StreamExtraction,
    diagnostic: Diagnostic,
) -> StreamExtraction {
    let mut result = extraction.result;
    result.status = ExtractionStatus::Malformed;
    result.byte_len = None;
    result.sha256 = None;
    result.diagnostics.push(diagnostic);
    StreamExtraction { result, data: None }
}

/// Decode modern Part B-Rep and display tessellation from in-memory bytes.
///
/// This capability is intentionally separate from [`parse_bytes`]: metadata
/// callers do not pay the geometry cost, and exact stream extraction remains
/// independently available through [`extract_bytes`].
#[must_use]
pub fn decode_geometry_bytes(
    data: &[u8],
    filename: Option<&str>,
    limits: &ResourceLimits,
) -> GeometryResult {
    geometry::decode(data, filename, SourceInputKind::Bytes, limits)
}

/// Read a bounded path and decode its modern Part geometry.
#[must_use]
pub fn decode_geometry_path(path: impl AsRef<Path>, limits: &ResourceLimits) -> GeometryResult {
    let path = path.as_ref();
    let label = path.to_string_lossy().into_owned();
    match read_path_bounded(path, limits) {
        Ok(data) => geometry::decode(&data, Some(&label), SourceInputKind::Path, limits),
        Err(failure) => GeometryResult {
            status: GeometryStatus::Rejected,
            geometry: None,
            diagnostics: vec![failure.diagnostic],
        },
    }
}

/// Build a source-faithful result from in-memory bytes.
///
/// The bounded semantic profiles decode supported source metadata and
/// references. Unsupported streams remain traceable and keep the result
/// partial.
#[must_use]
pub fn parse_bytes(data: &[u8], filename: Option<&str>, limits: &ResourceLimits) -> ParseResult {
    parse_bytes_with_source(data, filename, SourceInputKind::Bytes, limits)
}

/// Read and parse a path while retaining path provenance in the result.
#[must_use]
pub fn parse_path(path: impl AsRef<Path>, limits: &ResourceLimits) -> ParseResult {
    let path = path.as_ref();
    let label = path.to_string_lossy().into_owned();
    match read_path_bounded(path, limits) {
        Ok(data) => parse_bytes_with_source(&data, Some(&label), SourceInputKind::Path, limits),
        Err(failure) => {
            let failure = *failure;
            ParseResult {
                status: ParseStatus::Rejected,
                document: None,
                inventory: None,
                diagnostics: vec![failure.diagnostic],
                coverage: empty_coverage(failure.total_bytes),
                semantic_coverage: None,
            }
        }
    }
}

fn parse_bytes_with_source(
    data: &[u8],
    filename: Option<&str>,
    input_kind: SourceInputKind,
    limits: &ResourceLimits,
) -> ParseResult {
    let inspection = inspect_bytes(data, filename, limits);
    match inspection.status {
        InventoryStatus::Rejected => ParseResult {
            status: ParseStatus::Rejected,
            document: None,
            inventory: inspection.inventory,
            diagnostics: inspection.diagnostics,
            coverage: inspection.coverage,
            semantic_coverage: None,
        },
        InventoryStatus::Malformed => ParseResult {
            status: ParseStatus::Malformed,
            document: None,
            inventory: inspection.inventory,
            diagnostics: inspection.diagnostics,
            coverage: inspection.coverage,
            semantic_coverage: None,
        },
        InventoryStatus::Unsupported if inspection.inventory.is_none() => ParseResult {
            status: ParseStatus::Unsupported,
            document: None,
            inventory: None,
            diagnostics: inspection.diagnostics,
            coverage: inspection.coverage,
            semantic_coverage: None,
        },
        InventoryStatus::Complete | InventoryStatus::Partial | InventoryStatus::Unsupported => {
            match inspection
                .inventory
                .as_ref()
                .map(|inventory| inventory.envelope)
            {
                Some(Envelope::ModernChunk) => {
                    recognized_modern(data, filename, input_kind, inspection, limits)
                }
                Some(Envelope::Ole2Cfb)
                    if inspection
                        .inventory
                        .as_ref()
                        .is_some_and(legacy_semantics::is_solidworks_candidate) =>
                {
                    recognized_legacy(data, filename, input_kind, inspection, limits)
                }
                _ => recognized_but_unsupported(data, filename, input_kind, inspection),
            }
        }
    }
}

fn recognized_modern(
    data: &[u8],
    filename: Option<&str>,
    input_kind: SourceInputKind,
    inspection: InventoryResult,
    limits: &ResourceLimits,
) -> ParseResult {
    let Some(inventory) = inspection.inventory.as_ref() else {
        return recognized_but_unsupported(data, filename, input_kind, inspection);
    };
    let filename_kind = document_kind_hint(filename);
    let facts = modern_semantics::decode(data, inventory, &filename_kind, limits);
    let mut diagnostics = inspection.diagnostics;
    diagnostics.extend(facts.diagnostics);
    if !facts.rejected {
        diagnostics.push(Diagnostic::new(
            "format.modern_profile_partial",
            DiagnosticSeverity::Info,
            DiagnosticKind::Unsupported,
            "this metadata profile excludes feature and geometry semantics; use the explicit geometry capability for modern Part geometry",
        ));
    }

    let byte_len = u64::try_from(data.len()).unwrap_or(u64::MAX);
    let document = SourceDocument {
        source: SourceInfo {
            input_kind,
            label: filename.map(str::to_owned),
            byte_len,
            sha256: sha256_hex(data),
        },
        envelope: SourceValue::new(
            Envelope::ModernChunk,
            ValueOrigin::Source,
            vec!["marker.modern_chunk_candidate".to_owned()],
        ),
        document_kind: facts.document_kind,
        internal_version: facts.internal_version,
        configurations: facts.configurations,
        properties: facts.properties,
        references: facts.references,
        preview: facts.preview,
        sheets: facts.sheets,
        unknown_records: facts.unknown_records,
    };

    ParseResult {
        status: if facts.rejected {
            ParseStatus::Rejected
        } else {
            ParseStatus::Partial
        },
        document: Some(document),
        inventory: inspection.inventory,
        diagnostics,
        coverage: inspection.coverage,
        semantic_coverage: Some(facts.semantic_coverage),
    }
}

fn recognized_legacy(
    data: &[u8],
    filename: Option<&str>,
    input_kind: SourceInputKind,
    inspection: InventoryResult,
    limits: &ResourceLimits,
) -> ParseResult {
    let Some(inventory) = inspection.inventory.as_ref() else {
        return recognized_but_unsupported(data, filename, input_kind, inspection);
    };
    let filename_kind = document_kind_hint(filename);
    let facts = legacy_semantics::decode(data, inventory, &filename_kind, limits);
    let mut diagnostics = inspection.diagnostics;
    diagnostics.extend(facts.diagnostics);
    if !facts.rejected {
        diagnostics.push(Diagnostic::new(
            "format.legacy_profile_partial",
            DiagnosticSeverity::Info,
            DiagnosticKind::Unsupported,
            "the current legacy profile decodes bounded metadata and previews; document features and geometry remain unsupported",
        ));
    }

    let byte_len = u64::try_from(data.len()).unwrap_or(u64::MAX);
    let document = SourceDocument {
        source: SourceInfo {
            input_kind,
            label: filename.map(str::to_owned),
            byte_len,
            sha256: sha256_hex(data),
        },
        envelope: SourceValue::new(
            Envelope::Ole2Cfb,
            ValueOrigin::Source,
            vec!["signature.ole2_cfb".to_owned()],
        ),
        document_kind: facts.document_kind,
        internal_version: facts.internal_version,
        configurations: facts.configurations,
        properties: facts.properties,
        references: Vec::new(),
        preview: facts.preview,
        sheets: Vec::new(),
        unknown_records: facts.unknown_records,
    };

    ParseResult {
        status: if facts.rejected {
            ParseStatus::Rejected
        } else {
            ParseStatus::Partial
        },
        document: Some(document),
        inventory: inspection.inventory,
        diagnostics,
        coverage: inspection.coverage,
        semantic_coverage: Some(facts.semantic_coverage),
    }
}

fn recognized_but_unsupported(
    data: &[u8],
    filename: Option<&str>,
    input_kind: SourceInputKind,
    inspection: InventoryResult,
) -> ParseResult {
    let envelope = inspection
        .inventory
        .as_ref()
        .map_or(Envelope::Unknown, |inventory| inventory.envelope);
    let (code, message) = match envelope {
        Envelope::ModernChunk => (
            "format.modern_semantics_unsupported",
            "modern SolidWorks document semantics are not supported for this layout",
        ),
        Envelope::Ole2Cfb => (
            "format.ole2_semantics_unsupported",
            "OLE2/CFB SolidWorks stream semantics are not supported",
        ),
        Envelope::ZipOpc => (
            "format.zip_semantics_unsupported",
            "ZIP/OPC SolidWorks content semantics are not supported",
        ),
        Envelope::Unknown => (
            "format.unrecognized",
            "input does not match a recognized SolidWorks container candidate",
        ),
    };

    let evidence = vec![
        match envelope {
            Envelope::ModernChunk => "marker.modern_chunk_candidate",
            Envelope::Ole2Cfb => "signature.ole2_cfb",
            Envelope::ZipOpc => "signature.zip",
            Envelope::Unknown => "format.unrecognized",
        }
        .to_owned(),
    ];
    let mut diagnostics = inspection.diagnostics;
    diagnostics.push(Diagnostic::new(
        code,
        DiagnosticSeverity::Warning,
        DiagnosticKind::Unsupported,
        message,
    ));

    let byte_len = u64::try_from(data.len()).unwrap_or(u64::MAX);
    let document = SourceDocument {
        source: SourceInfo {
            input_kind,
            label: filename.map(str::to_owned),
            byte_len,
            sha256: sha256_hex(data),
        },
        envelope: SourceValue::new(envelope, ValueOrigin::Source, evidence),
        document_kind: document_kind_hint(filename),
        internal_version: None,
        configurations: Vec::new(),
        properties: Vec::new(),
        references: Vec::new(),
        preview: None,
        sheets: Vec::new(),
        unknown_records: Vec::new(),
    };

    ParseResult {
        status: ParseStatus::Unsupported,
        document: Some(document),
        inventory: inspection.inventory,
        diagnostics,
        coverage: inspection.coverage,
        semantic_coverage: None,
    }
}

fn append_extension_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    filename: Option<&str>,
    recognized: bool,
) {
    if !recognized {
        return;
    }
    let Some(extension) = filename
        .and_then(|value| Path::new(value).extension())
        .and_then(|extension| extension.to_str())
    else {
        return;
    };
    if matches!(
        extension.to_ascii_lowercase().as_str(),
        "sldprt" | "sldasm" | "slddrw"
    ) {
        return;
    }
    diagnostics.push(
        Diagnostic::new(
            "input.extension_mismatch",
            DiagnosticSeverity::Warning,
            DiagnosticKind::Preserved,
            "recognized container content takes precedence over the filename extension",
        )
        .with_detail("extension", extension)
        .with_detail("classification_basis", "content"),
    );
}

fn document_kind_hint(filename: Option<&str>) -> SourceValue<DocumentKind> {
    let kind = filename.map_or(DocumentKind::Unknown, |value| {
        match Path::new(value)
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("sldprt") => DocumentKind::Part,
            Some("sldasm") => DocumentKind::Assembly,
            Some("slddrw") => DocumentKind::Drawing,
            _ => DocumentKind::Unknown,
        }
    });

    let evidence = if kind == DocumentKind::Unknown {
        Vec::new()
    } else {
        vec!["filename.extension".to_owned()]
    };
    SourceValue::new(kind, ValueOrigin::Hint, evidence)
}

fn sha256_hex(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(data);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

struct ReadFailure {
    diagnostic: Diagnostic,
    total_bytes: u64,
}

fn read_path_bounded(path: &Path, limits: &ResourceLimits) -> Result<Vec<u8>, Box<ReadFailure>> {
    let file = File::open(path).map_err(|error| Box::new(io_failure(path, &error)))?;
    let metadata = file
        .metadata()
        .map_err(|error| Box::new(io_failure(path, &error)))?;
    if metadata.len() > limits.max_file_size {
        return Err(Box::new(limit_failure(
            path,
            metadata.len(),
            limits.max_file_size,
        )));
    }

    let read_limit = limits.max_file_size.saturating_add(1);
    let mut data = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut data)
        .map_err(|error| Box::new(io_failure(path, &error)))?;

    let actual_bytes = u64::try_from(data.len()).unwrap_or(u64::MAX);
    if actual_bytes > limits.max_file_size {
        return Err(Box::new(limit_failure(
            path,
            actual_bytes,
            limits.max_file_size,
        )));
    }
    Ok(data)
}

fn io_failure(path: &Path, error: &std::io::Error) -> ReadFailure {
    ReadFailure {
        diagnostic: Diagnostic::new(
            "input.io",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "input path could not be read",
        )
        .with_detail("path", display_path(path))
        .with_detail("error_kind", format!("{:?}", error.kind())),
        total_bytes: 0,
    }
}

fn limit_failure(path: &Path, actual_bytes: u64, limit_bytes: u64) -> ReadFailure {
    ReadFailure {
        diagnostic: Diagnostic::new(
            "limit.file_size",
            DiagnosticSeverity::Error,
            DiagnosticKind::Fatal,
            "input exceeds the configured file-size limit",
        )
        .with_detail("path", display_path(path))
        .with_detail("actual_bytes", actual_bytes.to_string())
        .with_detail("limit_bytes", limit_bytes.to_string()),
        total_bytes: actual_bytes,
    }
}

fn display_path(path: &Path) -> String {
    PathBuf::from(path).to_string_lossy().into_owned()
}

fn rejected_probe(failure: ReadFailure) -> ProbeResult {
    ProbeResult {
        status: ProbeStatus::Rejected,
        envelope: Envelope::Unknown,
        confidence: ProbeConfidence::None,
        evidence: Vec::new(),
        diagnostics: vec![failure.diagnostic],
        coverage: empty_coverage(failure.total_bytes),
    }
}

fn rejected_inventory(failure: ReadFailure) -> InventoryResult {
    let total_bytes = failure.total_bytes;
    InventoryResult {
        status: InventoryStatus::Rejected,
        inventory: None,
        diagnostics: vec![failure.diagnostic],
        coverage: empty_coverage(total_bytes),
        uninterpreted_ranges: if total_bytes == 0 {
            Vec::new()
        } else {
            vec![sldkit_core::ByteRange::new(0, total_bytes)]
        },
    }
}

const fn empty_coverage(total_bytes: u64) -> CoverageReport {
    CoverageReport {
        total_bytes,
        inspected_bytes: 0,
        decoded_bytes: 0,
        uninterpreted_bytes: total_bytes,
        streams_total: 0,
        streams_decoded: 0,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use cfb::Version;
    use crc32fast::Hasher as Crc32;
    use flate2::{Compression, write::DeflateEncoder};
    use sldkit_core::{
        BinaryResource, BinaryResourceKind, DiagnosticKind, DocumentKind, Envelope,
        ExtractionStatus, ParseStatus, PropertyKind, PropertyValueState, ReferenceKind,
        ResourceLimits, SourceInputKind, ValueOrigin,
    };
    use tempfile::NamedTempFile;

    use super::{extract_resource_bytes, inspect_bytes, parse_bytes, parse_path, sha256_hex};

    const EMPTY_ZIP: &[u8] =
        b"PK\x05\x06\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";

    fn modern_file(streams: &[(&str, &[u8])]) -> Result<Vec<u8>, std::io::Error> {
        let mut output = b"SLDK\x00\x00\x00\x04".to_vec();
        for (name, payload) in streams {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(payload)?;
            let compressed = encoder.finish()?;
            let encoded_name = name
                .as_bytes()
                .iter()
                .map(|value| value.rotate_right(4))
                .collect::<Vec<_>>();
            output.extend_from_slice(b"\x14\x00\x06\x00\x08\x00");
            output.extend_from_slice(&7_u32.to_le_bytes());
            output.extend_from_slice(&crc32fast::hash(payload).to_le_bytes());
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
            output.extend_from_slice(
                &u32::try_from(encoded_name.len())
                    .unwrap_or(u32::MAX)
                    .to_le_bytes(),
            );
            output.extend_from_slice(&encoded_name);
            output.extend_from_slice(&compressed);
        }
        Ok(output)
    }

    fn push_archive_class(
        output: &mut Vec<u8>,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        output.extend_from_slice(&u16::MAX.to_le_bytes());
        output.extend_from_slice(&1_u16.to_le_bytes());
        output.extend_from_slice(&u16::try_from(name.len())?.to_le_bytes());
        output.extend_from_slice(name.as_bytes());
        Ok(())
    }

    fn push_archive_utf16_string(
        output: &mut Vec<u8>,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let units = value.encode_utf16().collect::<Vec<_>>();
        let length = u8::try_from(units.len())?;
        if matches!(length, 0xfe | 0xff) {
            return Err("test string requires an unsupported extended length".into());
        }
        output.extend_from_slice(&[0xff, 0xfe, 0xff, length]);
        for unit in units {
            output.extend_from_slice(&unit.to_le_bytes());
        }
        Ok(())
    }

    fn configuration_manager_header_19000(
        records: &[(u32, &str, Option<u32>)],
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut output = Vec::new();
        push_archive_class(&mut output, "dmConfigMgrHeader_c")?;
        output.extend_from_slice(&u16::try_from(records.len())?.to_le_bytes());
        for (record_number, (index, name, parent_index)) in records.iter().enumerate() {
            if record_number == 0 {
                push_archive_class(&mut output, "dmConfigHeader_c")?;
            } else {
                output.extend_from_slice(&0x8003_u16.to_le_bytes());
            }
            output.extend_from_slice(&u32::from(parent_index.is_some()).to_le_bytes());
            push_archive_utf16_string(&mut output, name)?;
            output.extend_from_slice(&index.to_le_bytes());
            output.extend_from_slice(&(100_u32.saturating_add(*index)).to_le_bytes());
            push_archive_utf16_string(&mut output, name)?;
            output.extend_from_slice(&parent_index.unwrap_or(u32::MAX).to_le_bytes());
            output.extend_from_slice(&0_u32.to_le_bytes());
            push_archive_utf16_string(&mut output, "")?;
            push_archive_utf16_string(&mut output, "")?;
            for _ in 0..4 {
                output.extend_from_slice(&0_u32.to_le_bytes());
            }
        }
        output.extend_from_slice(&[0; 8]);
        Ok(output)
    }

    fn preview_png() -> Vec<u8> {
        fn append_chunk(output: &mut Vec<u8>, kind: [u8; 4], data: &[u8]) {
            output.extend_from_slice(&u32::try_from(data.len()).unwrap_or(u32::MAX).to_be_bytes());
            output.extend_from_slice(&kind);
            output.extend_from_slice(data);
            let mut crc = Crc32::new();
            crc.update(&kind);
            crc.update(data);
            output.extend_from_slice(&crc.finalize().to_be_bytes());
        }

        let mut output = b"\x89PNG\r\n\x1a\n".to_vec();
        append_chunk(
            &mut output,
            *b"IHDR",
            b"\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00",
        );
        append_chunk(&mut output, *b"IDAT", b"x");
        append_chunk(&mut output, *b"IEND", b"");
        output
    }

    fn summary_information(author: &str, include_code_page: bool) -> Vec<u8> {
        const SUMMARY_FMTID: [u8; 16] = [
            0xe0, 0x85, 0x9f, 0xf2, 0xf9, 0x4f, 0x68, 0x10, 0xab, 0x91, 0x08, 0x00, 0x2b, 0x27,
            0xb3, 0xd9,
        ];

        let mut code_page = Vec::new();
        code_page.extend_from_slice(&0x0002_u16.to_le_bytes());
        code_page.extend_from_slice(&0_u16.to_le_bytes());
        code_page.extend_from_slice(&65001_u16.to_le_bytes());
        code_page.extend_from_slice(&0_u16.to_le_bytes());

        let mut author_value = Vec::new();
        author_value.extend_from_slice(&0x001e_u16.to_le_bytes());
        author_value.extend_from_slice(&0_u16.to_le_bytes());
        let author_bytes = author.as_bytes();
        author_value.extend_from_slice(
            &u32::try_from(author_bytes.len().saturating_add(1))
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        author_value.extend_from_slice(author_bytes);
        author_value.push(0);
        while !author_value.len().is_multiple_of(4) {
            author_value.push(0);
        }

        let property_count = if include_code_page { 2_u32 } else { 1_u32 };
        let table_end = 8_usize + usize::try_from(property_count).unwrap_or(0) * 8;
        let author_offset = table_end
            + if include_code_page {
                code_page.len()
            } else {
                0
            };
        let section_size = author_offset + author_value.len();
        let mut section = Vec::with_capacity(section_size);
        section.extend_from_slice(
            &u32::try_from(section_size)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        section.extend_from_slice(&property_count.to_le_bytes());
        if include_code_page {
            section.extend_from_slice(&1_u32.to_le_bytes());
            section.extend_from_slice(&u32::try_from(table_end).unwrap_or(u32::MAX).to_le_bytes());
        }
        section.extend_from_slice(&4_u32.to_le_bytes());
        section.extend_from_slice(
            &u32::try_from(author_offset)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        if include_code_page {
            section.extend_from_slice(&code_page);
        }
        section.extend_from_slice(&author_value);

        let mut output = Vec::with_capacity(48 + section.len());
        output.extend_from_slice(&0xfffe_u16.to_le_bytes());
        output.extend_from_slice(&0_u16.to_le_bytes());
        output.extend_from_slice(&0_u32.to_le_bytes());
        output.extend_from_slice(&[0; 16]);
        output.extend_from_slice(&1_u32.to_le_bytes());
        output.extend_from_slice(&SUMMARY_FMTID);
        output.extend_from_slice(&48_u32.to_le_bytes());
        output.extend_from_slice(&section);
        output
    }

    fn configuration_manager_header_2200() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut output = Vec::new();
        push_archive_class(&mut output, "dmConfigMgrHeader_c")?;
        output.extend_from_slice(&1_u16.to_le_bytes());
        push_archive_class(&mut output, "dmConfigHeader_c")?;
        output.extend_from_slice(&0_u32.to_le_bytes());
        push_archive_utf16_string(&mut output, "Default")?;
        output.extend_from_slice(&0_u32.to_le_bytes());
        output.extend_from_slice(&100_u32.to_le_bytes());
        push_archive_utf16_string(&mut output, "Default")?;
        output.extend_from_slice(&u32::MAX.to_le_bytes());
        Ok(output)
    }

    fn preview_dib() -> Vec<u8> {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40_u32.to_le_bytes());
        dib.extend_from_slice(&1_i32.to_le_bytes());
        dib.extend_from_slice(&1_i32.to_le_bytes());
        dib.extend_from_slice(&1_u16.to_le_bytes());
        dib.extend_from_slice(&24_u16.to_le_bytes());
        dib.extend_from_slice(&0_u32.to_le_bytes());
        dib.extend_from_slice(&4_u32.to_le_bytes());
        dib.extend_from_slice(&0_i32.to_le_bytes());
        dib.extend_from_slice(&0_i32.to_le_bytes());
        dib.extend_from_slice(&0_u32.to_le_bytes());
        dib.extend_from_slice(&0_u32.to_le_bytes());

        let mut output = Vec::new();
        output.extend_from_slice(&44_u32.to_le_bytes());
        output.extend_from_slice(&dib);
        output.extend_from_slice(&[0; 4]);
        output
    }

    fn legacy_file(summary: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let cursor = Cursor::new(Vec::new());
        let mut compound = cfb::CompoundFile::create_with_version(Version::V3, cursor)?;
        compound.create_storage("/Contents")?;
        compound.create_storage("/_MO_VERSION_2200")?;
        {
            let mut stream = compound.create_stream("/\u{5}SummaryInformation")?;
            stream.write_all(summary)?;
        }
        {
            let mut stream = compound.create_stream("/Contents/CMgrHdr2")?;
            stream.write_all(&configuration_manager_header_2200()?)?;
        }
        {
            let mut stream = compound.create_stream("/Contents/Config-0")?;
            stream.write_all(b"opaque configuration data")?;
        }
        {
            let mut stream = compound.create_stream("/Preview")?;
            stream.write_all(&preview_dib())?;
        }
        compound.flush()?;
        Ok(compound.into_inner().into_inner())
    }

    fn generic_ole_file() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let cursor = Cursor::new(Vec::new());
        let mut compound = cfb::CompoundFile::create_with_version(Version::V3, cursor)?;
        {
            let mut stream = compound.create_stream("/Document")?;
            stream.write_all(b"generic compound-file payload")?;
        }
        compound.flush()?;
        Ok(compound.into_inner().into_inner())
    }

    fn assert_resource_extracts(input: &[u8], resource: &BinaryResource, expected: &[u8]) {
        let extraction = extract_resource_bytes(input, resource, &ResourceLimits::desktop());
        assert_eq!(extraction.result.status, ExtractionStatus::Extracted);
        assert_eq!(extraction.data.as_deref(), Some(expected));
        assert_eq!(extraction.result.byte_len, Some(resource.byte_len));
        assert_eq!(
            extraction.result.sha256.as_deref(),
            Some(resource.sha256.as_str())
        );
    }

    #[test]
    fn resource_extraction_revalidates_stream_range_and_digest()
    -> Result<(), Box<dyn std::error::Error>> {
        let decoded = b"prefix-preview-suffix";
        let input = modern_file(&[("Contents/Preview", decoded)])?;
        let inventory = inspect_bytes(&input, None, &ResourceLimits::desktop());
        let entry = inventory
            .inventory
            .as_ref()
            .and_then(|value| value.entries.first())
            .ok_or("inventory entry missing")?;
        let resource = BinaryResource {
            kind: BinaryResourceKind::PreviewPng,
            entry_id: entry.id.clone(),
            stream_path: "Contents/Preview".to_owned(),
            decoded_offset: 7,
            byte_len: 7,
            sha256: sha256_hex(b"preview"),
            media_type: "image/png".to_owned(),
        };

        let extracted = extract_resource_bytes(&input, &resource, &ResourceLimits::desktop());
        assert_eq!(extracted.result.status, ExtractionStatus::Extracted);
        assert_eq!(extracted.data.as_deref(), Some(b"preview".as_slice()));

        let mut wrong_path = resource.clone();
        wrong_path.stream_path = "Contents/Other".to_owned();
        let mismatch = extract_resource_bytes(&input, &wrong_path, &ResourceLimits::desktop());
        assert_eq!(mismatch.result.status, ExtractionStatus::Malformed);
        assert_eq!(
            mismatch.result.diagnostics[0].code,
            "extract.resource_stream_mismatch"
        );
        assert!(mismatch.data.is_none());

        let mut invalid_range = resource.clone();
        invalid_range.decoded_offset = u64::MAX;
        invalid_range.byte_len = 2;
        let invalid = extract_resource_bytes(&input, &invalid_range, &ResourceLimits::desktop());
        assert_eq!(invalid.result.status, ExtractionStatus::Malformed);
        assert_eq!(
            invalid.result.diagnostics[0].code,
            "extract.resource_range_invalid"
        );

        let mut wrong_digest = resource;
        wrong_digest.sha256 = "0".repeat(64);
        let mismatch = extract_resource_bytes(&input, &wrong_digest, &ResourceLimits::desktop());
        assert_eq!(mismatch.result.status, ExtractionStatus::Malformed);
        assert_eq!(
            mismatch.result.diagnostics[0].code,
            "extract.resource_sha256_mismatch"
        );
        Ok(())
    }

    #[test]
    fn recognized_envelope_is_not_reported_as_semantically_parsed() {
        let result = parse_bytes(EMPTY_ZIP, Some("part.SLDPRT"), &ResourceLimits::desktop());
        assert_eq!(result.status, ParseStatus::Unsupported);
        assert_eq!(result.diagnostics[0].kind, DiagnosticKind::Unsupported);

        let document = result.document.as_ref();
        assert!(document.is_some());
        if let Some(document) = document {
            assert_eq!(document.envelope.value, Envelope::ZipOpc);
            assert_eq!(document.document_kind.value, DocumentKind::Part);
            assert_eq!(document.document_kind.origin, ValueOrigin::Hint);
            assert_eq!(document.source.input_kind, SourceInputKind::Bytes);
        }
    }

    #[test]
    fn generic_compound_file_is_not_claimed_as_a_solidworks_document()
    -> Result<(), Box<dyn std::error::Error>> {
        let input = generic_ole_file()?;
        let result = parse_bytes(
            &input,
            Some("not-solidworks.SLDPRT"),
            &ResourceLimits::service(),
        );

        assert_eq!(result.status, ParseStatus::Unsupported);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "format.ole2_semantics_unsupported"
                && diagnostic.kind == DiagnosticKind::Unsupported
        }));
        let document = result.document.as_ref().ok_or("document missing")?;
        assert_eq!(document.envelope.value, Envelope::Ole2Cfb);
        assert!(document.properties.is_empty());
        Ok(())
    }

    #[test]
    fn legacy_metadata_configuration_and_dib_preview_are_source_faithful()
    -> Result<(), Box<dyn std::error::Error>> {
        let input = legacy_file(&summary_information("Ada", true))?;
        let result = parse_bytes(&input, Some("fixture.SLDPRT"), &ResourceLimits::service());

        assert_eq!(result.status, ParseStatus::Partial);
        let document = result.document.as_ref().ok_or("document missing")?;
        assert_eq!(document.envelope.value, Envelope::Ole2Cfb);
        assert_eq!(document.document_kind.value, DocumentKind::Part);
        assert_eq!(document.document_kind.origin, ValueOrigin::Hint);
        assert_eq!(
            document
                .internal_version
                .as_ref()
                .map(|version| (version.value, version.origin)),
            Some((2_200, ValueOrigin::Source))
        );
        assert_eq!(document.configurations.len(), 1);
        assert_eq!(
            document.configurations[0]
                .name
                .as_ref()
                .map(|name| (name.value.as_str(), name.origin)),
            Some(("Default", ValueOrigin::Source))
        );

        let author = document
            .properties
            .iter()
            .find(|property| property.name.value == "Author")
            .ok_or("author property missing")?;
        assert_eq!(author.kind, PropertyKind::Core);
        assert_eq!(author.value_state, PropertyValueState::Present);
        assert_eq!(
            author.raw_value.as_ref().map(|value| value.value.as_str()),
            Some("Ada")
        );

        let preview_resource = document.preview.as_ref().ok_or("preview missing")?;
        assert_eq!(preview_resource.kind, BinaryResourceKind::PreviewDib);
        let preview = preview_dib();
        assert_resource_extracts(&input, preview_resource, &preview[4..]);
        assert!(document.references.is_empty());
        assert!(document.sheets.is_empty());
        assert!(document.unknown_records.iter().any(|record| {
            record.stream_path.as_deref() == Some("/Contents/Config-0")
                && record.reason_code == "legacy.configuration.unsupported"
        }));

        let semantic = result
            .semantic_coverage
            .ok_or("semantic coverage missing")?;
        assert_eq!(
            semantic.fully_interpreted_streams
                + semantic.partially_interpreted_streams
                + semantic.uninterpreted_streams
                + semantic.malformed_streams,
            semantic.decoded_streams_total
        );
        assert!(!result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "legacy.property_type_or_encoding_unsupported"
        }));
        Ok(())
    }

    #[test]
    fn legacy_ascii_property_without_code_page_has_one_precise_diagnostic()
    -> Result<(), Box<dyn std::error::Error>> {
        let input = legacy_file(&summary_information("Ada", false))?;
        let result = parse_bytes(&input, Some("fixture.SLDPRT"), &ResourceLimits::service());
        let document = result.document.as_ref().ok_or("document missing")?;
        assert!(document.properties.iter().any(|property| {
            property.name.value == "Author"
                && property
                    .raw_value
                    .as_ref()
                    .is_some_and(|value| value.value == "Ada")
        }));
        assert_eq!(
            result
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == "legacy.property_code_page_missing")
                .count(),
            1
        );
        assert!(!result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "legacy.property_type_or_encoding_unsupported"
        }));
        Ok(())
    }

    #[test]
    fn malformed_legacy_property_stream_is_not_an_empty_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let input = legacy_file(&[0xfe, 0xff])?;
        let result = parse_bytes(&input, Some("fixture.SLDPRT"), &ResourceLimits::service());

        assert_eq!(result.status, ParseStatus::Partial);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "legacy.property_set_header_truncated"
                && diagnostic.kind == DiagnosticKind::Malformed
        }));
        let document = result.document.as_ref().ok_or("document missing")?;
        assert!(document.properties.is_empty());
        assert!(document.unknown_records.iter().any(|record| {
            record.stream_path.as_deref() == Some("/\u{5}SummaryInformation")
                && record.reason_code == "legacy.property_set.malformed"
        }));
        assert_eq!(
            result
                .semantic_coverage
                .map(|coverage| coverage.malformed_streams),
            Some(1)
        );
        Ok(())
    }

    #[test]
    fn malformed_and_unsupported_results_remain_distinct() {
        let malformed = parse_bytes(&[], None, &ResourceLimits::desktop());
        let unsupported = parse_bytes(b"plain text", None, &ResourceLimits::desktop());
        assert_eq!(malformed.status, ParseStatus::Malformed);
        assert_eq!(unsupported.status, ParseStatus::Unsupported);
        assert_eq!(malformed.diagnostics[0].kind, DiagnosticKind::Malformed);
        assert_eq!(unsupported.diagnostics[0].kind, DiagnosticKind::Unsupported);
    }

    #[test]
    fn path_entry_point_records_path_provenance() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = NamedTempFile::with_suffix(".SLDASM")?;
        file.write_all(EMPTY_ZIP)?;

        let result = parse_path(file.path(), &ResourceLimits::desktop());
        let document = result.document.as_ref();
        assert!(document.is_some());
        if let Some(document) = document {
            assert_eq!(document.source.input_kind, SourceInputKind::Path);
            assert_eq!(document.document_kind.value, DocumentKind::Assembly);
        }
        Ok(())
    }

    #[test]
    fn path_read_honors_file_size_limit() -> Result<(), Box<dyn std::error::Error>> {
        let mut file = NamedTempFile::new()?;
        file.write_all(b"four")?;
        let mut limits = ResourceLimits::service();
        limits.max_file_size = 3;

        let result = parse_path(file.path(), &limits);
        assert_eq!(result.status, ParseStatus::Rejected);
        assert_eq!(result.diagnostics[0].code, "limit.file_size");
        Ok(())
    }

    #[test]
    fn modern_metadata_preserves_property_states_and_source_evidence()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = br#"<root><swHeader><swFile id="0" swDocType="PART"/><swFile id="1" swDocType="PART" swPath="C:\source\linked.SLDPRT"/></swHeader><swModelList><swModel swConfigurationId="0"><swConfiguration swID="0" swName="Default" swParentConfigurationName="MissingParent"/></swModel></swModelList></root>"#;
        let properties = br#"<root><propertySection name="UserDefinedProperties"><property name="present" pid="2"><lpwstr>value</lpwstr></property><property name="empty"><lpwstr></lpwstr></property><property name="missing"/><property name="opaque"><blob>0102</blob></property></propertySection></root>"#;
        let config_properties = br#"<root><propertySection name="UserDefinedProperties"><property name="Configuration"><lpwstr>Default</lpwstr></property></propertySection></root>"#;
        let system = br#"<root><propertySection name="System"><property name="SW-MassProp-Config-0"><lpstr>1,2,3,4,5,6,7,8,9,10,11,12,13</lpstr></property></propertySection></root>"#;
        let preview = preview_png();
        let input = modern_file(&[
            ("swXmlContents/Features", model),
            ("docProps/custom.xml", properties),
            ("docProps/Config-0-Properties.xml", config_properties),
            ("docProps/ISolidWorksInformation.xml", system),
            ("PreviewPNG", &preview),
            ("Config-0-PreviewPNG", &preview),
            ("_MO_VERSION_18000/Contents", b"opaque"),
            ("Contents/Unknown", b"preserve me"),
        ])?;

        let result = parse_bytes(&input, Some("wrong.SLDASM"), &ResourceLimits::desktop());
        assert_eq!(result.status, ParseStatus::Partial);
        let document = result.document.as_ref().ok_or("document missing")?;
        assert_eq!(document.document_kind.value, DocumentKind::Part);
        assert_eq!(document.document_kind.origin, ValueOrigin::Source);
        assert_eq!(
            document.internal_version.as_ref().map(|value| value.value),
            Some(18_000)
        );
        assert!(document.preview.is_some());
        let preview_resource = document.preview.as_ref().ok_or("preview missing")?;
        assert_resource_extracts(&input, preview_resource, &preview);
        assert_eq!(document.configurations.len(), 1);
        let configuration = &document.configurations[0];
        assert_eq!(configuration.index.value, 0);
        assert_eq!(
            configuration.name.as_ref().map(|name| name.value.as_str()),
            Some("Default")
        );
        assert!(configuration.preview.is_some());
        assert_eq!(
            configuration
                .mass_properties
                .as_ref()
                .map(|mass| mass.mass.as_str()),
            Some("6")
        );
        assert_eq!(
            configuration
                .mass_properties
                .as_ref()
                .map(|mass| mass.additional_values.as_slice()),
            Some(["13".to_owned()].as_slice())
        );

        let states = document
            .properties
            .iter()
            .filter(|property| {
                matches!(
                    property.name.value.as_str(),
                    "present" | "empty" | "missing" | "opaque"
                )
            })
            .map(|property| {
                (
                    property.name.value.as_str(),
                    property.value_state,
                    property
                        .raw_value
                        .as_ref()
                        .map(|value| value.value.as_str()),
                )
            })
            .collect::<Vec<_>>();
        assert!(states.contains(&("present", PropertyValueState::Present, Some("value"))));
        assert!(states.contains(&("empty", PropertyValueState::Empty, Some(""))));
        assert!(states.contains(&("missing", PropertyValueState::Missing, None)));
        assert!(states.contains(&("opaque", PropertyValueState::UnsupportedType, Some("0102"))));
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| { item.code == "modern.configuration_parent_unresolved" })
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| item.code == "input.document_kind_mismatch")
        );
        assert!(
            document
                .unknown_records
                .iter()
                .any(|record| record.stream_path.as_deref() == Some("Contents/Unknown"))
        );
        Ok(())
    }

    #[test]
    fn modern_assembly_and_drawing_references_remain_source_native()
    -> Result<(), Box<dyn std::error::Error>> {
        let assembly = br#"<root><swHeader><swFile id="0" swDocType="ASSEMBLY"/><swFile id="1" swDocType="PART" swPath="C:\pack\child.SLDPRT"/></swHeader><swModelList><swModel id="m0" swFileRef="0" swConfigurationId="0"><swConfiguration swID="0" swName="Default"/><swReference swModelRef="m1" swName="child-1" swSuppressed="NO" swHidden="YES"/></swModel><swModel id="m1" swFileRef="1" swConfigurationName="Machined"/></swModelList></root>"#;
        let assembly_input = modern_file(&[("swXmlContents/COMPINSTANCETREE", assembly)])?;
        let assembly_result = parse_bytes(
            &assembly_input,
            Some("fixture.SLDASM"),
            &ResourceLimits::desktop(),
        );
        let assembly_document = assembly_result
            .document
            .as_ref()
            .ok_or("document missing")?;
        let component = assembly_document.configurations[0]
            .components
            .first()
            .ok_or("component missing")?;
        assert_eq!(
            component
                .stored_path
                .as_ref()
                .map(|value| value.value.as_str()),
            Some(r"C:\pack\child.SLDPRT")
        );
        assert_eq!(
            component
                .referenced_configuration
                .as_ref()
                .map(|value| value.value.as_str()),
            Some("Machined")
        );
        assert_eq!(
            component.is_suppressed.as_ref().map(|value| value.value),
            Some(false)
        );
        assert_eq!(
            component.is_hidden.as_ref().map(|value| value.value),
            Some(true)
        );
        assert!(component.exclude_from_bom.is_none());
        assert_eq!(
            assembly_document.references[0].kind,
            ReferenceKind::AssemblyComponent
        );

        let keywords = br#"<root><Sheet Type="Sheet" id="s1" Name="Sheet1"><View id="v1" Name="Front" Description="Machined">child.SLDPRT</View></Sheet></root>"#;
        let mut sheet_names = b"\x01\x00\xff\xfe\xff\x06".to_vec();
        for unit in "Sheet1".encode_utf16() {
            sheet_names.extend_from_slice(&unit.to_le_bytes());
        }
        let preview = preview_png();
        let drawing_input = modern_file(&[
            ("swXmlContents/KeyWords", keywords),
            ("SheetPreviews/SheetNames", &sheet_names),
            ("Images/Sheet_0", &preview),
        ])?;
        let drawing_result = parse_bytes(
            &drawing_input,
            Some("fixture.SLDDRW"),
            &ResourceLimits::desktop(),
        );
        let drawing_document = drawing_result.document.as_ref().ok_or("document missing")?;
        assert_eq!(drawing_document.document_kind.value, DocumentKind::Drawing);
        assert!(drawing_document.configurations.is_empty());
        assert_eq!(drawing_document.sheets.len(), 1);
        assert_eq!(drawing_document.sheets[0].views.len(), 1);
        assert!(drawing_document.sheets[0].preview.is_some());
        assert_eq!(
            drawing_document.references[0].kind,
            ReferenceKind::DrawingView
        );
        Ok(())
    }

    #[test]
    fn configuration_manager_parent_and_relocated_assembly_roots_are_decoded()
    -> Result<(), Box<dyn std::error::Error>> {
        let assembly = br#"<swSolidWorks><swHeader>
            <swFile id="old-self" swDocType="ASSEMBLY" swPath="C:\pack\TEST.SLDASM"/>
            <swFile id="part-1" swDocType="PART" swPath="C:\pack\TEST1.SLDPRT"/>
            <swFile id="part-2" swDocType="PART" swPath="C:\pack\TEST2.SLDPRT"/>
            <swFile id="new-self" swDocType="ASSEMBLY" swPath="C:\pack\TEST2.SLDASM"/>
            </swHeader><swModelList>
            <swModel id="part-model-1" swFileRef="part-1" swConfigurationName="Default"/>
            <swModel id="part-model-2" swFileRef="part-2" swConfigurationName="Default"/>
            <swModel id="root-default" swFileRef="old-self" swConfigurationId="0">
              <swReference swModelRef="part-model-1" swName="TEST1" swSuppressed="NO"/>
              <swReference swModelRef="part-model-2" swName="TEST2" swSuppressed="NO"/>
            </swModel>
            <swModel id="root-base" swFileRef="old-self" swConfigurationId="1">
              <swReference swModelRef="part-model-1" swName="TEST1" swSuppressed="NO"/>
              <swReference swModelRef="part-model-2" swName="TEST2" swSuppressed="NO"/>
            </swModel>
            <swModel id="root-derived" swFileRef="new-self" swConfigurationId="2">
              <swReference swModelRef="part-model-1" swName="TEST1" swSuppressed="NO"/>
              <swReference swModelRef="part-model-2" swName="TEST2" swSuppressed="YES"/>
            </swModel>
            </swModelList><swConfigurationList>
            <swConfiguration swID="0" swName="Default" swModelRef="root-default"/>
            <swConfiguration swID="1" swName="Base" swModelRef="root-base"/>
            <swConfiguration swID="2" swName="Derived-Suppressed" swModelRef="root-derived"/>
            </swConfigurationList></swSolidWorks>"#;
        let configuration_manager = configuration_manager_header_19000(&[
            (0, "Default", None),
            (1, "Base", None),
            (2, "Derived-Suppressed", Some(1)),
        ])?;
        let input = modern_file(&[
            ("Contents/CMgrHdr2", &configuration_manager),
            ("swXmlContents/COMPINSTANCETREE", assembly),
            ("_MO_VERSION_19000/Biography", b"opaque"),
        ])?;

        let result = parse_bytes(&input, Some("TEST2.SLDASM"), &ResourceLimits::desktop());
        let document = result.document.as_ref().ok_or("document missing")?;
        assert_eq!(document.configurations.len(), 3);
        let derived = document
            .configurations
            .iter()
            .find(|configuration| configuration.index.value == 2)
            .ok_or("derived configuration missing")?;
        assert_eq!(
            derived.parent_index.as_ref().map(|value| value.value),
            Some(1)
        );
        assert_eq!(
            derived
                .parent_name
                .as_ref()
                .map(|value| (value.value.as_str(), value.origin)),
            Some(("Base", ValueOrigin::Derived))
        );
        assert_eq!(derived.components.len(), 2);
        assert_eq!(
            derived.components[0]
                .is_suppressed
                .as_ref()
                .map(|value| value.value),
            Some(false)
        );
        assert_eq!(
            derived.components[1]
                .is_suppressed
                .as_ref()
                .map(|value| value.value),
            Some(true)
        );
        assert_eq!(document.references.len(), 6);
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "modern.configuration_root_model_relocated"
                && diagnostic
                    .details
                    .get("configuration_index")
                    .map(String::as_str)
                    == Some("2")
        }));
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "modern.configuration_manager_trailing_bytes_preserved"
        }));
        Ok(())
    }

    #[test]
    fn truncated_configuration_manager_header_is_not_silently_empty()
    -> Result<(), Box<dyn std::error::Error>> {
        let configuration_manager = configuration_manager_header_19000(&[
            (0, "Default", None),
            (1, "Base", None),
            (2, "Derived-Suppressed", Some(1)),
        ])?;
        let truncated = configuration_manager
            .get(..configuration_manager.len() / 2)
            .ok_or("truncation range missing")?;
        let input = modern_file(&[
            ("Contents/CMgrHdr2", truncated),
            ("_MO_VERSION_19000/Biography", b"opaque"),
        ])?;

        let result = parse_bytes(&input, Some("TEST2.SLDASM"), &ResourceLimits::service());
        assert!(result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "modern.configuration_manager_truncated"
                && diagnostic.kind == DiagnosticKind::Malformed
        }));
        Ok(())
    }

    #[test]
    fn localized_drawing_sheet_type_is_recovered_from_direct_views()
    -> Result<(), Box<dyn std::error::Error>> {
        let keywords = r#"<root><Sheet Type="ｼｰﾄ" id="s1" Name="ｼｰﾄ1"><View id="v1" Name="図面ﾋﾞｭｰ1" Description="ﾃﾞﾌｫﾙﾄ">TEST.SLDPRT</View></Sheet><Sheet Type="ｼｰﾄ ﾌｫｰﾏｯﾄ" id="format" Name="ｼｰﾄ ﾌｫｰﾏｯﾄ1"/></root>"#;
        let input = modern_file(&[("swXmlContents/KeyWords", keywords.as_bytes())])?;

        let result = parse_bytes(&input, Some("fixture.SLDDRW"), &ResourceLimits::desktop());
        let document = result.document.as_ref().ok_or("document missing")?;

        assert_eq!(document.document_kind.value, DocumentKind::Drawing);
        assert_eq!(document.sheets.len(), 1);
        assert_eq!(
            document.sheets[0]
                .name
                .as_ref()
                .map(|value| value.value.as_str()),
            Some("ｼｰﾄ1")
        );
        assert_eq!(document.sheets[0].views.len(), 1);
        assert_eq!(
            document.sheets[0].views[0]
                .referenced_document
                .as_ref()
                .map(|value| value.value.as_str()),
            Some("TEST.SLDPRT")
        );
        assert_eq!(document.references.len(), 1);
        assert!(result.diagnostics.iter().any(|item| {
            item.code == "modern.drawing_sheet_type_inferred_from_views"
                && item.details.get("source_type").map(String::as_str) == Some("ｼｰﾄ")
        }));
        Ok(())
    }

    #[test]
    fn malformed_semantic_stream_is_diagnostic_not_an_empty_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let model = br#"<root><swHeader><swFile swDocType="PART"/></swHeader></root>"#;
        let input = modern_file(&[
            ("swXmlContents/Features", model),
            ("docProps/custom.xml", b"<root><propertySection>"),
            ("PreviewPNG", b"not a PNG"),
        ])?;

        let result = parse_bytes(&input, Some("fixture.SLDPRT"), &ResourceLimits::desktop());
        assert_eq!(result.status, ParseStatus::Partial);
        let document = result.document.as_ref().ok_or("document missing")?;
        assert_eq!(document.document_kind.value, DocumentKind::Part);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| item.code == "modern.xml_malformed")
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| item.code == "modern.preview_png_missing_signature")
        );
        assert!(document.unknown_records.iter().any(|record| {
            record.reason_code == "semantic.stream_malformed"
                && record.stream_path.as_deref() == Some("docProps/custom.xml")
        }));
        assert_eq!(
            result
                .semantic_coverage
                .map(|coverage| coverage.malformed_streams),
            Some(2)
        );
        Ok(())
    }

    #[test]
    fn semantic_xml_size_limit_rejects_before_tree_construction()
    -> Result<(), Box<dyn std::error::Error>> {
        let input = modern_file(&[(
            "swXmlContents/Features",
            b"<root><swHeader><swFile swDocType=\"PART\"/></swHeader></root>",
        )])?;
        let mut limits = ResourceLimits::service();
        limits.max_xml_stream_bytes = 16;

        let result = parse_bytes(&input, Some("fixture.SLDPRT"), &limits);

        assert_eq!(result.status, ParseStatus::Rejected);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|item| item.code == "limit.xml_stream_bytes")
        );
        assert_eq!(
            result
                .semantic_coverage
                .map(|coverage| coverage.malformed_streams),
            Some(1)
        );
        Ok(())
    }
}
