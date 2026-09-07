# sldkit backend patches 1 through 4

This is the Apache-2.0 `cadmpeg-codec-sldprt` 0.5.3 registry source, patched
as `0.5.3+sldkit.4`. Other cadmpeg dependencies remain pinned by the workspace
lockfile. The path dependency is intentional: published wheels and sdists must
contain this implementation, without a developer-local Cargo override.

- Upstream: <https://github.com/cadmpeg/cadmpeg/tree/v0.5.3/crates/cadmpeg-codec-sldprt>
- Upstream git revision: `dbb308e52ab05c911b26963a02fbb294eefab3ce`
- Registry archive SHA-256: `d02f9ee3bc25aa04c7ffa9747b7f0778aaac2114976fd97195909c7418dfac50`
- License: [Apache-2.0](LICENSE). Upstream source notices are retained.

## Parsing changes

`brep/native_hierarchy.rs` independently reads native BODY (12), REGION (19),
and SHELL (13) links for **`SCH_3701229_37102_13006` only**. Both embedded BODY
and REGION declarations must match the verified field definitions. Body kind
1 is solid, kind 3 is sheet. Face membership comes from shell-linked native
face IDs, with reciprocal owner/previous links, unique identities, and complete
coverage of the effective face table. Coordinates, connected components, body
names, fixture hashes, and expected oracle counts do not select membership.

`brep/entity.rs`, `brep.rs`, and `brep/graph.rs` integrate that result. Exterior
void regions are checked but not emitted as additional material regions; a
sheet's void region owns its open boundary. Native shells are not divided by
geometric connectivity. Explicit byte-exact annotations identify the source
body/region/shell records. These annotations remain location anchors. Patch 2 supplies exact reads in a
separate sidecar; it does not reinterpret anchors as ranges.

The reader requires one partition with consistent schemas. Duplicate, cyclic,
dangling, incomplete, or unsupported records leave the previous partial
decoder behavior in place. Recognized hierarchy records in deltas disable the
recovery: hierarchy-update ordering is not implemented. Wire/general bodies,
alternate declarations/framing, and other schema versions are outside this
profile; arbitrary newer SolidWorks files are not verified support.

`history/mod.rs` reads native `ConfigurationManager` IDs without assuming XML
list order. Conflicting explicit/native IDs remain unresolved. `decode.rs`
joins only unique source IDs to available partitions, preserves unresolved
membership for native ConfigurationManager records with missing partitions,
and uses the unique saved
`swMostRecentConfiguration` ID before the legacy active-name fallback.
The upstream SourceIndex-only writer profile retains its existing convention
of encoding empty configurations by omitting their partition stream.

Evidence is a project-authored SolidWorks 2026 SP0.0 Part, independently
captured SolidWorks API values, and per-configuration STEP references. Native
file SHA-256: `fb9fa08e2991870ec193c49578fb195fa9a830c0fec79b6d10496c4c328a2571`.
Private CAD files and capture artifacts are excluded from distribution.
Synthetic tests exercise malformed links and sparse/reordered configuration
IDs; they are distinct from the real-file acceptance evidence.

## Patch 2: exact read ledger

`SldprtCodec::decode_with_byte_ledger` returns the ordinary decode result plus
a version-1 `ByteLedger`. Every domain retains its physical site, nested stream
ordinal/outer offset, description, schema, full-stream/body lengths and hashes.
Readers record consumed fields before partition/deltas merging. Spans have
body-relative start/length, reader tag, local native record ID, classification,
and a hash of the exact bytes. Standard admission/finalization still applies.

`brep/topology.rs`, analytic carrier readers, and `brep/spline.rs` record
successful reads. NURBS wrapper/descriptor/array spans remain non-contiguous;
reserved float slots beyond semantic knot counts are excluded. Ambiguous
array origins have no typed span. `native_hierarchy.rs` also records validated
BODY/REGION declarations and consumed fields, including exterior void records.

This is a bounded instrumentation profile, not universal Parasolid framing.
Uninstrumented reader families and unknown fields supply no typed span. The
sldkit adapter independently verifies bytes/domain identity, unions typed
reads, and retains the complement as uninterpreted. Complete interval coverage
does not upgrade model exactness, geometric coverage, or supported schemas.
No geometry, entity IDs, writer semantics, or configuration selection changes
are intended by this patch.

## Patch 3: native FIN orientation

`brep/native_fin.rs` converts the inherited base FIN fields to the legacy graph
builder's internal convention, only after the exact native hierarchy profile
has passed. Source FIN `refs[2]`/`refs[3]` are forward/backward links and `refs[4]`
is the forward (end) vertex. The graph builder had treated them as previous/next
and a start vertex, reversing the published boundary traversal and endpoints.
The conversion swaps the traversal slots and obtains each start vertex from
the opposite FIN. It preserves the source sense markers, offsets, read spans,
carrier definitions, and original retained bytes. STEP is not an input to it.

The semantic reference is Siemens' **Parasolid XT Format Reference, April 2008,
FIN, pp. 96–97**, available in this
[public copy](https://ww3.cad.de/foren/ubb/uploads/Rainer%2BSchulze/XT_Format_April_2008_tcm73-62642.pdf).
The current native sample also supplies reciprocal forward/backward links,
opposite senses/edge identities, end-vertex joins, and hidden sheet-boundary
fins. Modern dummy FIN 75 participates in the same-vertex chain (unlike the
older manual's null-only dummy description); its target's vertex is verified.
The numeric IDs are evidence, never selection rules in the implementation.

Every FIN pair/ring/vertex link is checked before any conversion. Missing,
inconsistent, non-manifold radial, or fin-local curve relationships withhold
the FIN arena and report `topology.native-fin-unresolved`. The graph and byte
ledger keep their separate contracts: withholding topology does not discard
read-domain evidence. Unverified schemas retain the existing compatibility
behavior. Source-less writer fixtures keep their original convention.

Controlled evidence: all 76 visible FINs across Base/Derived match independent
source-byte reads; the four NURBS boundary uses now follow the pinned STEP wire
without diagnostic reversal. Numeric trim ranges and outer/inner loop roles
are not fabricated. The inspected FIN fields contain vertices and sense, not
a numeric trim interval; derived support-endpoint intervals remain a distinct
validation result. `source_trim_gate_passed` remains false.

## Patch 4: shared Parasolid readers

The runtime depends on published `parasolid-core = "=0.1.0-dev6"`. Compact
analytic records, topology, native hierarchy/FIN validation, spline arrays and
point/spline patch helpers, intersection charts, sweep/spin, blend, offset and
subset payload readers now live in `parasolid_core::partial`. Their duplicate
implementations and extracted layout constants are removed here. The retained
files adapt neutral records to CadIr, scale source lengths to millimetres, and
convert exact read spans to the existing byte ledger.

All container/configuration processing, attribute/history recovery, derived
surface and pcurve construction, and tessellation stay in this adapter. The
partial core preserves the existing limited deltas merge; it does not interpret
unknown delta type 3/4 as a complete framed stream or certify final state. The
strict core schema parser remains a separate, fail-closed API. No failed shared
reader is retried through a private copy.

The transferred readers and tests retain Apache-2.0 notices. The core's
`PARTIAL_READERS.md` records hashes of this patch-3 input and the modifications.
The aggregate core also includes original MIT code; this adapter remains
Apache-2.0 licensed. Backend version changes to `0.5.3+sldkit.4`; geometry,
configuration, IDs, units, diagnostics and read-ledger contracts are preserved.

## Test portability

The registry archive includes unit-test sources but omits the upstream golden
fixtures and unpublished `cadmpeg-test-support` dependency. `lib.rs` leaves the
golden module unregistered; its original source is retained. Two unit tests in
`metadata_fallback.rs` and `extrusion_profile.rs` use direct preserved-write
byte comparisons in place of the unavailable helper, retaining those checks.
No upstream golden-corpus result is claimed.

Run the available upstream and patch unit tests from the repository root:

```sh
CARGO_TARGET_DIR=target cargo test --locked --manifest-path vendor/cadmpeg-codec-sldprt/Cargo.toml --lib
cargo fmt --manifest-path vendor/cadmpeg-codec-sldprt/Cargo.toml -- --check
```
