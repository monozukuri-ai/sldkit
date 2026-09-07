//! Validate decoder reads against independently extracted nested streams.

use std::collections::{BTreeMap, BTreeSet};

use cadmpeg_codec_sldprt::byte_ledger::{ByteLedger, SpanClassification};
use sldkit_core::{
    GeometryByteClassification as Class, GeometryByteDomain, GeometryByteRange, GeometryDecodedSpan,
};

#[derive(Debug, Default)]
pub(super) struct Partition {
    pub spans: Vec<GeometryDecodedSpan>,
    pub ranges: Vec<GeometryByteRange>,
    pub typed_bytes: u64,
    pub uninterpreted_bytes: u64,
}

pub(super) fn site_key(entry: &str) -> Result<String, String> {
    let offset = entry
        .strip_prefix("modern:block:")
        .ok_or("unknown entry kind")?;
    let offset = u64::from_str_radix(offset, 16).map_err(|_| "invalid entry offset")?;
    Ok(format!("block@{offset}"))
}

/// `None` preserves incomplete coverage for unavailable or unsupported ledgers.
#[allow(clippy::too_many_lines)]
pub(super) fn normalize(
    domains: &[GeometryByteDomain],
    streams: &BTreeMap<String, Vec<u8>>,
    ledger: &ByteLedger,
) -> Result<Option<Partition>, String> {
    if domains.is_empty() || ledger.version != 1 || ledger.domains.is_empty() {
        return Ok(None);
    }
    let mut expected = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut selected_sites = BTreeSet::new();
    for domain in domains {
        let site = site_key(&domain.container_entry_id)?;
        selected_sites.insert(site.clone());
        if !ids.insert(&domain.id)
            || expected
                .insert((site, domain.outer_payload_offset), domain)
                .is_some()
        {
            return Err("duplicate extracted domain".into());
        }
    }
    let mut matched = BTreeMap::new();
    let mut ordinals = BTreeSet::new();
    for actual in &ledger.domains {
        if !selected_sites.contains(&actual.site_key) {
            return Err("ledger contains an unknown source site".into());
        }
        let key = (actual.site_key.clone(), actual.outer_payload_offset);
        let domain = expected.get(&key).ok_or("unknown ledger domain")?;
        if matched.insert(&domain.id, actual).is_some()
            || !ordinals.insert((&actual.site_key, actual.nested_ordinal))
        {
            return Err("duplicate ledger domain".into());
        }
        if actual.stream_path != domain.stream_path
            || actual.description != domain.description
            || actual.schema != domain.schema
            || actual.stream_byte_len != domain.stream_byte_len
            || actual.stream_sha256 != domain.stream_sha256
            || actual.body_offset != domain.body_offset
            || actual.byte_len != domain.byte_len
            || actual.sha256 != domain.sha256
        {
            return Err(format!("ledger identity mismatch: {}", domain.id));
        }
    }
    if matched.len() != domains.len() {
        return Err("ledger is missing an extracted domain".into());
    }
    let mut result = Partition::default();
    for domain in domains {
        let actual = matched.get(&domain.id).ok_or("missing ledger domain")?;
        let stream = streams.get(&domain.id).ok_or("missing extracted bytes")?;
        let body_start = usize::try_from(domain.body_offset).map_err(|_| "body offset overflow")?;
        let body = stream.get(body_start..).ok_or("body outside stream")?;
        if u64::try_from(stream.len()).ok() != Some(domain.stream_byte_len)
            || u64::try_from(body.len()).ok() != Some(domain.byte_len)
            || super::sha256_hex(stream) != domain.stream_sha256
            || super::sha256_hex(body) != domain.sha256
        {
            return Err("extracted bytes do not match domain".into());
        }
        let mut typed = Vec::new();
        let mut unknown = Vec::new();
        for span in &actual.spans {
            let end = span
                .offset
                .checked_add(span.byte_len)
                .ok_or("span overflow")?;
            if span.byte_len == 0 || end > domain.byte_len || span.tag.is_empty() {
                return Err("empty or out-of-domain span".into());
            }
            let start = usize::try_from(span.offset).map_err(|_| "span offset overflow")?;
            let finish = usize::try_from(end).map_err(|_| "span end overflow")?;
            if super::sha256_hex(&body[start..finish]) != span.sha256 {
                return Err("span hash mismatch".into());
            }
            let classification = match span.classification {
                SpanClassification::Typed => {
                    typed.push((span.offset, end));
                    Class::Typed
                }
                SpanClassification::Uninterpreted => {
                    unknown.push((span.offset, end));
                    Class::Uninterpreted
                }
            };
            result.spans.push(GeometryDecodedSpan {
                domain_id: domain.id.clone(),
                offset: span.offset,
                byte_len: span.byte_len,
                classification,
                tag: span.tag.clone(),
                source_record_id: span.source_record_id,
                sha256: span.sha256.clone(),
            });
        }
        let typed = union(typed);
        let unknown = union(unknown);
        let mut unknown_index = 0;
        for &(start, end) in &typed {
            while unknown
                .get(unknown_index)
                .is_some_and(|range| range.1 <= start)
            {
                unknown_index += 1;
            }
            if unknown
                .get(unknown_index)
                .is_some_and(|range| range.0 < end)
            {
                return Err("typed and uninterpreted spans overlap".into());
            }
        }
        let mut cursor = 0;
        for (start, end) in typed {
            append(&mut result, &domain.id, cursor, start, Class::Uninterpreted)?;
            append(&mut result, &domain.id, start, end, Class::Typed)?;
            cursor = end;
        }
        append(
            &mut result,
            &domain.id,
            cursor,
            domain.byte_len,
            Class::Uninterpreted,
        )?;
    }
    result.spans.sort_by(|left, right| {
        (
            &left.domain_id,
            left.offset,
            left.byte_len,
            &left.tag,
            left.source_record_id,
        )
            .cmp(&(
                &right.domain_id,
                right.offset,
                right.byte_len,
                &right.tag,
                right.source_record_id,
            ))
    });
    Ok(Some(result))
}

fn union(mut intervals: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    intervals.sort_unstable();
    let mut result: Vec<(u64, u64)> = Vec::new();
    for (start, end) in intervals {
        if let Some(last) = result.last_mut().filter(|last| start <= last.1) {
            last.1 = last.1.max(end);
        } else {
            result.push((start, end));
        }
    }
    result
}

fn append(
    result: &mut Partition,
    domain: &str,
    start: u64,
    end: u64,
    class: Class,
) -> Result<(), String> {
    if start == end {
        return Ok(());
    }
    let count = end.checked_sub(start).ok_or("reversed interval")?;
    let (counter, reason) = match class {
        Class::Typed => (&mut result.typed_bytes, "decoder_confirmed_fields"),
        Class::Uninterpreted => (
            &mut result.uninterpreted_bytes,
            "not_decoded_by_range_aware_readers",
        ),
    };
    *counter = counter.checked_add(count).ok_or("byte count overflow")?;
    result.ranges.push(GeometryByteRange {
        domain_id: domain.into(),
        offset: start,
        byte_len: count,
        classification: class,
        reason: reason.into(),
    });
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::type_complexity)]
mod tests {
    use super::*;
    use cadmpeg_codec_sldprt::byte_ledger::{ByteDomain, DecodedSpan};
    use sldkit_core::{GeometryByteOffsetBasis, GeometryByteStorage, GeometryStreamRole};

    fn fixture() -> (
        Vec<GeometryByteDomain>,
        BTreeMap<String, Vec<u8>>,
        ByteLedger,
    ) {
        let mut domains = Vec::new();
        let mut streams = BTreeMap::new();
        let mut ledger = ByteLedger {
            version: 1,
            domains: Vec::new(),
        };
        // The same record offset in partition and deltas is two independent reads.
        for i in 0..2 {
            let bytes: Vec<u8> = (0..16).map(|n| n + i).collect();
            let domain = GeometryByteDomain {
                id: format!("domain-{i}"),
                container_entry_id: "modern:block:00000100".into(),
                stream_path: "Contents/Partition".into(),
                role: GeometryStreamRole::ParasolidPartition,
                storage: GeometryByteStorage::Direct,
                outer_payload_offset: u64::from(i) * 32,
                description: if i == 0 { "partition" } else { "deltas" }.into(),
                schema: "test".into(),
                stream_byte_len: 16,
                stream_sha256: crate::geometry::sha256_hex(&bytes),
                body_offset: 2,
                byte_len: 14,
                sha256: crate::geometry::sha256_hex(&bytes[2..]),
                offset_basis: GeometryByteOffsetBasis::ParasolidBody,
            };
            ledger.domains.push(ByteDomain {
                site_key: "block@256".into(),
                stream_path: domain.stream_path.clone(),
                nested_ordinal: usize::from(i),
                outer_payload_offset: domain.outer_payload_offset,
                description: domain.description.clone(),
                schema: domain.schema.clone(),
                stream_byte_len: domain.stream_byte_len,
                stream_sha256: domain.stream_sha256.clone(),
                body_offset: domain.body_offset,
                byte_len: domain.byte_len,
                sha256: domain.sha256.clone(),
                spans: vec![
                    span(&bytes[2..], 2, 5),
                    span(&bytes[2..], 4, 5),
                    span(&bytes[2..], 11, 1),
                ],
            });
            streams.insert(domain.id.clone(), bytes);
            domains.push(domain);
        }
        (domains, streams, ledger)
    }

    fn span(body: &[u8], offset: u64, byte_len: u64) -> DecodedSpan {
        DecodedSpan {
            classification: SpanClassification::Typed,
            offset,
            byte_len,
            tag: "test.read".into(),
            source_record_id: Some(42),
            sha256: crate::geometry::sha256_hex(
                &body
                    [usize::try_from(offset).unwrap()..usize::try_from(offset + byte_len).unwrap()],
            ),
        }
    }

    #[test]
    fn unions_reads_and_retains_gaps_without_merging_domains() {
        let (domains, streams, ledger) = fixture();
        let result = normalize(&domains, &streams, &ledger).unwrap().unwrap();
        assert_eq!((result.typed_bytes, result.uninterpreted_bytes), (16, 12));
        assert_eq!(result.spans.len(), 6);
        for domain in &domains {
            let ranges: Vec<_> = result
                .ranges
                .iter()
                .filter(|r| r.domain_id == domain.id)
                .collect();
            assert_eq!(
                ranges
                    .iter()
                    .map(|r| (r.offset, r.byte_len, r.classification))
                    .collect::<Vec<_>>(),
                vec![
                    (0, 2, Class::Uninterpreted),
                    (2, 7, Class::Typed),
                    (9, 2, Class::Uninterpreted),
                    (11, 1, Class::Typed),
                    (12, 2, Class::Uninterpreted),
                ]
            );
        }
    }

    #[test]
    fn rejects_bad_spans_and_domain_identity() {
        let (domains, streams, original) = fixture();
        let mutations: Vec<Box<dyn Fn(&mut ByteLedger)>> = vec![
            Box::new(|l| l.domains[0].spans[0].offset = u64::MAX),
            Box::new(|l| l.domains[0].spans[0].byte_len = 0),
            Box::new(|l| l.domains[0].spans[0].byte_len = 100),
            Box::new(|l| l.domains[0].spans[0].sha256.clear()),
            Box::new(|l| l.domains[0].schema.push('x')),
            Box::new(|l| l.domains[0].stream_sha256.clear()),
            Box::new(|l| l.domains[0].body_offset += 1),
            Box::new(|l| l.domains[0].site_key = "block@999".into()),
            Box::new(|l| l.domains[0].outer_payload_offset = 999),
            Box::new(|l| l.domains[1].nested_ordinal = 0),
            Box::new(|l| {
                l.domains.push(l.domains[0].clone());
            }),
            Box::new(|l| {
                l.domains.pop();
            }),
            Box::new(|l| l.domains[0].spans[1].classification = SpanClassification::Uninterpreted),
        ];
        for (index, mutate) in mutations.iter().enumerate() {
            let mut ledger = original.clone();
            mutate(&mut ledger);
            assert!(
                normalize(&domains, &streams, &ledger).is_err(),
                "mutation {index}"
            );
        }
        let mut changed_bytes = streams.clone();
        changed_bytes.get_mut("domain-0").unwrap()[0] ^= 1;
        assert!(normalize(&domains, &changed_bytes, &original).is_err());
        let mut duplicate = domains.clone();
        duplicate.push(domains[0].clone());
        assert!(normalize(&duplicate, &streams, &original).is_err());
    }

    #[test]
    fn unsupported_ledger_stays_incomplete_but_empty_reads_are_uninterpreted() {
        let (domains, streams, mut ledger) = fixture();
        ledger.version = 2;
        assert!(normalize(&domains, &streams, &ledger).unwrap().is_none());
        ledger.version = 1;
        for domain in &mut ledger.domains {
            domain.spans.clear();
        }
        let result = normalize(&domains, &streams, &ledger).unwrap().unwrap();
        assert_eq!((result.typed_bytes, result.uninterpreted_bytes), (0, 28));
        ledger.domains.clear();
        assert!(normalize(&domains, &streams, &ledger).unwrap().is_none());
    }
}
