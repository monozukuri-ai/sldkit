// SPDX-License-Identifier: Apache-2.0
//! Exact, body-relative reads from the range-aware geometry readers.
//!
//! A span describes decoded fields, not necessarily a complete native record.
//! Uninstrumented fields and records have no typed span. Consumers may retain
//! their complement as uninterpreted bytes without claiming semantic coverage.

use std::cell::RefCell;

use cadmpeg_core::{
    decode::{DecodeContext, View},
    CodecError, ContainerSummary, ReadSeek,
};
use cadmpeg_ir::codec::{Codec, CodecBackend, Confidence, DecodeOptions, DecodeResult};
use serde::{Deserialize, Serialize};

use crate::SldprtCodec;

/// Meaning of a decoder-provided interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanClassification {
    /// Decoded fields with known byte boundaries.
    Typed,
    /// An explicitly retained field whose interpretation is unavailable.
    Uninterpreted,
}

/// A decoder-confirmed field interval in one Parasolid body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedSpan {
    /// Typed reads or an explicit uninterpreted interval.
    pub classification: SpanClassification,
    /// Body-relative first byte, inclusive.
    pub offset: u64,
    /// Exact number of bytes read, never the distance to another record.
    pub byte_len: u64,
    /// Reader field or framing class.
    pub tag: String,
    /// Native carrier/record identity scoped to this domain, when available.
    pub source_record_id: Option<u16>,
    /// Digest of the exact span bytes, populated when the domain is finalized.
    pub sha256: String,
}

impl DecodedSpan {
    pub(crate) fn new(start: usize, end: usize, tag: &str, source_record_id: Option<u16>) -> Self {
        Self {
            classification: SpanClassification::Typed,
            offset: start as u64,
            byte_len: end.saturating_sub(start) as u64,
            tag: tag.into(),
            source_record_id,
            sha256: String::new(),
        }
    }
}

impl From<parasolid_core::partial::ReadSpan> for DecodedSpan {
    fn from(span: parasolid_core::partial::ReadSpan) -> Self {
        Self::new(
            span.offset,
            span.offset + span.byte_len,
            &span.tag,
            span.source_record_id,
        )
    }
}

/// One independent nested stream before partition/deltas facts are merged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteDomain {
    /// Outer block/compound identity from the actual decoder input.
    pub site_key: String,
    /// Exact containing stream path.
    pub stream_path: String,
    /// Ordinal within the containing entry's extracted Parasolid streams.
    pub nested_ordinal: usize,
    /// Start of the direct stream or zlib member in the outer payload.
    pub outer_payload_offset: u64,
    /// Exact source description, including partition/deltas role.
    pub description: String,
    /// Exact source schema token.
    pub schema: String,
    /// Full inflated stream size.
    pub stream_byte_len: u64,
    /// Full inflated stream digest.
    pub stream_sha256: String,
    /// Body start relative to the inflated stream.
    pub body_offset: u64,
    /// Body size.
    pub byte_len: u64,
    /// Body digest.
    pub sha256: String,
    /// Exact typed reads; overlap and non-contiguous carrier fields are valid.
    pub spans: Vec<DecodedSpan>,
}

/// Versioned sidecar; it is independent of neutral entity annotations.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteLedger {
    /// Reader contract version (currently 1).
    pub version: u32,
    /// Domains in deterministic source order.
    pub domains: Vec<ByteDomain>,
}

struct LedgerBackend(RefCell<ByteLedger>);

impl CodecBackend for LedgerBackend {
    fn id(&self) -> &'static str {
        "sldprt"
    }
    fn detect(&self, prefix: &[u8]) -> Confidence {
        SldprtCodec.detect(prefix)
    }
    fn inspect_impl(
        &self,
        ctx: &DecodeContext<'_>,
        root: View<'_>,
    ) -> Result<ContainerSummary, CodecError> {
        SldprtCodec.inspect_impl(ctx, root)
    }
    fn decode_impl(
        &self,
        ctx: &DecodeContext<'_>,
        root: View<'_>,
    ) -> Result<DecodeResult, CodecError> {
        crate::decode::decode_with_ledger(ctx, root, &mut self.0.borrow_mut())
    }
}

impl SldprtCodec {
    /// Decode with the usual admission/finalization policy and an exact-read
    /// sidecar. This does not change the ordinary `Codec::decode` IR contract.
    pub fn decode_with_byte_ledger(
        &self,
        reader: &mut dyn ReadSeek,
        options: &DecodeOptions,
    ) -> Result<(DecodeResult, ByteLedger), CodecError> {
        let backend = LedgerBackend(RefCell::new(ByteLedger {
            version: 1,
            domains: Vec::new(),
        }));
        let decoded = backend.decode(reader, options)?;
        Ok((decoded, backend.0.into_inner()))
    }
}
