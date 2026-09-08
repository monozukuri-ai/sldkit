# Modern Part geometry

Geometry decoding is an explicit capability for modern chunk-form `.SLDPRT`
files. It is separate from metadata parsing so callers can inspect properties
and references without paying the geometry cost or accepting a geometry
dependency in their data flow.

## Parasolid dependency

`sldkit-parser` and its SolidWorks adapter use the published
`parasolid-core = "=0.1.0-dev6"` Rust crate. No Python `parasolid-kit` package or
adjacent checkout is required. Standard embedded `X_B` headers are validated
under the caller's size/string limits. The public `parasolid_body` origin remains
immediately after the schema string; offsets and hashes retain their meaning.
The two source-less cadmpeg writer keys retain their explicit local header reader.

The production path calls `parasolid_core::partial` for compact topology,
validated BODY/REGION/SHELL ownership, FIN normalization, analytic/NURBS carriers,
intersection caches, sweep/spin, offset, blend, subset records, and exact read
spans where instrumented. Point and NURBS patch helpers are shared too. The old
copies of these readers have been removed; there is no second geometry decode
or retry through them. Core values stay in source units; the adapter converts
model-space lengths to millimetres exactly once and preserves wire IDs/offsets.

Container extraction, configuration/stream pairing, public models, source hashes,
byte-partition classification, attributes/history, pcurve/patch construction,
and tessellation remain in sldkit's `cadmpeg-codec-sldprt 0.5.3+sldkit.4` adapter.
The backend version identifies this combined pipeline. No foreign Rust types
are exposed in the public sldkit model.

The partial API preserves the existing bounded partition/deltas merge; it does
not establish arbitrary topology replacement/deletion or full saved-state
reconstruction. The strict core `parse_xb`/B-Rep path still accepts the exact
`SCH_3701229_37102_13006` partition profile and rejects unknown delta base types
3/4. Partial record recognition does not upgrade that strict status, geometry
exactness, source trim evidence, or byte coverage. Derived intersection caches
and pcurves retain their existing classification. See the core's
[partial-reader contract and provenance](https://docs.rs/crate/parasolid-core/0.1.0-dev6/source/PARTIAL_READERS.md).

## Entry points

Python exposes `decode_geometry_file` and `decode_geometry_bytes`:

```python
import sldkit

result = sldkit.decode_geometry_file("part.SLDPRT", profile="service")
if result.geometry is not None:
    model = result.geometry.model
    print(len(model.bodies), len(model.faces), len(model.tessellations))
    for body in result.geometry.topology_metrics:
        print(body.body_id, body.faces, body.edges, body.vertices)
for loss in result.geometry.fidelity.losses if result.geometry else ():
    print(loss.code, loss.category, loss.severity)
```

The Rust API provides `decode_geometry_path` and `decode_geometry_bytes`. Both
command-line interfaces provide a `geometry` command:

```bash
sldkit geometry part.SLDPRT --limits service
sldkit-rs geometry part.SLDPRT --limits service
```

`strict=True` accepts only `GeometryStatus.DECODED`. A partial, unsupported,
malformed, or rejected result raises `GeometryError` and keeps the structured
result on the exception.

## Source model

The geometry model keeps explicit topology ownership from body through vertex:

- bodies own regions;
- regions own shells;
- shells own faces, wire edges, and free vertices;
- faces identify their support surface and loops;
- loops identify coedges and ordered vertex uses;
- coedges identify their edge, neighbors, radial use, sense, and pcurves;
- edges identify their curve and endpoint vertices; and
- vertices identify source points.

A body with `provenance.tag == "synthetic_grouping"` and derived exactness
is a decoder grouping. Its count and kind do not establish the number or
solid/sheet classification of native bodies. Inspect provenance and losses
before using body counts or Euler values as solid-validity checks.

Surface, curve, and pcurve carriers have a domain, a source carrier kind, a
tagged parameter object, and entity provenance. Analytic carriers and NURBS are
returned without tessellating them into a replacement shape. Procedural
carriers link to a separate construction record, preserving both the solved
carrier and its source construction. Pcurves also retain wrapper direction,
native tail flags, range, and fit tolerance when present. An untyped carrier or
construction links to matching retained bytes through `raw_record_id` when the
decoder retained the native record, rather than approximating it.

Native object identity and effective display state remain attached to carriers,
points, and tessellations; body and face display state remains on those topology
entities. Tessellations stay separate from B-Rep carriers and preserve triangle
groups and texture assignment identities. Face ownership can be absent when it
cannot be established; the loss report records that condition. Opaque
tessellation channels are summarized by their metadata, SHA-256, and byte
length instead of being copied into JSON.

Configuration body membership uses three states: a list is resolved
membership, an empty list is resolved absence, and `None` is unresolved.

## Stream identity and fidelity

`source_streams` lists exact outer-container entries considered during decode.
Each entry includes the inventory entry ID, source path, decoded size and
SHA-256, semantic role, selection state, and selection evidence. The same
entry ID can be passed to `extract_file` or `extract_bytes` to retrieve and
verify the decoded source stream independently of semantic decoding.

`active` means a source used by the geometry decoder, not necessarily the
configuration currently selected in SolidWorks. Multiple partition entries
can be active when the model contains geometry from multiple configurations.
Entity provenance can establish this participation only when the stream path
identifies one decoded inventory entry; duplicate paths remain unresolved.
Nested stream discovery shares its byte, stream-count, and inflate-attempt
budgets across contributing entries. `geometry.byte_domain_limit` reports when
those budgets are exhausted; the returned domains then cover only a subset.

Entity provenance records a stream, byte offset, source tag, and exactness when
the decoder established them. Missing exactness evidence is `unknown`, never
silently upgraded to byte-exact.

The fidelity report contains:

- whether typed geometry was transferred;
- entity counts and topology validation findings;
- categorized loss records with source locations where available;
- exact candidate and active outer-stream byte counts;
- exact nested Parasolid byte-domain identities and hashes; and
- retained-record byte counts.

These byte counters have different domains. Decoded stream sizes can differ
from compressed file ranges, and retained records can overlap other retained
records. Therefore they must not be summed into a source-file partition.

`byte_domains` identifies each partition or deltas body nested inside an active
outer entry. A domain records whether the complete `PS` stream was direct or
zlib-wrapped, its offset in the decoded outer payload, the complete-stream and
body sizes and SHA-256 digests, and the header-to-body offset. Entity provenance
offsets use the `parasolid_body` basis. Consequently,
`partition_domain_bytes` may be larger than `active_stream_bytes` after nested
decompression.

`located_entity_count` counts typed entities with an active-stream source
location; `unique_location_count` deduplicates their `(stream, offset)` pairs.
Locations are anchors, not byte ranges. `classified_active_bytes` and
`unclassified_active_bytes` partition `partition_domain_bytes`; the two field
names are retained for compatibility with the initial API. When exact source
record ranges are unavailable, all geometry-domain bytes remain unclassified,
`partition_status` is `incomplete`, and both `typed_bytes` and
`uninterpreted_bytes` are `None`. The result also contains the
`geometry.byte_partition_incomplete` diagnostic. A location anchor is never
promoted into a one-byte typed range.

The patched backend also supplies a versioned read ledger. `byte_spans` exposes
exact field reads with `domain_id`, body-relative `offset` / `byte_len`, reader
`tag`, domain-local `source_record_id`, classification, and SHA-256. A NURBS
carrier has separate wrapper, descriptor, and array spans. Overlapping typed
reads are normalized into `byte_ranges`; the retained complement is explicitly
`uninterpreted` with reason `not_decoded_by_range_aware_readers`.

Each ledger domain must match independently extracted stream and body hashes,
schema, description, containing entry, and nested stream offset. Missing,
duplicate, conflicting, or out-of-bounds spans/domains reject the result with
`geometry.byte_ledger_invalid`. Unsupported ledger versions or exhausted
nested extraction budgets leave coverage incomplete. Exact ranges never come
from the distance between entity anchors.

For a verified ledger, `partition_status=complete`, `typed_bytes` plus
`uninterpreted_bytes` equals `partition_domain_bytes`, and unclassified bytes
are zero. **Complete partition means exhaustive byte accounting, not complete
semantic decoding or exact geometry.** The range-aware readers cover accepted
analytic carriers, NURBS curves/surfaces and their arrays, topology field reads,
and the bounded native hierarchy profile below. Other reader families,
unrecognized records, unused fields, and reserved NURBS float slots remain
uninterpreted. Entity provenance stays unchanged; native IDs in spans are
local source IDs, not neutral IR entity IDs.


## Independent geometry validation

Neutral B-Rep reference values can be recorded against
[`schemas/geometry-oracle.schema.json`](schemas/geometry-oracle.schema.json).
The contract keeps artifact hashes, the source revision, reference topology,
mass properties, bounds, and tolerances together. Topology comparison is
separate from geometric comparison because a neutral export may split or merge
faces and edges without changing the represented solid.

`scripts/capture_step_geometry.py` captures reference values with an optional
OCP installation. OCP is a validation-time tool, not a package dependency.
`scripts/validate_geometry_oracle.py` then checks the pinned artifact hashes and
compares those values with volume, area, center of mass, and bounds computed
from the native Part's decoded display tessellation. Tolerances are supplied by
the oracle record and are never selected from the observed result.

STEP capture counts explicit solids, free shells, and free faces separately,
without sewing or healing. Free shells remain sheet representations even if
closed; an export can therefore have different body kinds from the native
document. Volume and volume-weighted center of mass include explicit solids
only, while surface area includes sheets. Capture metadata records this scope.

## Numerical NURBS comparison

`scripts/validate_nurbs_geometry.py` compares the native NURBS support carriers
referenced by a named configuration against STEP B-splines. Run it in a
validation environment with OCP and the repository's Python development tools:

```sh
python scripts/validate_nurbs_geometry.py oracle.json fixture-root --output result.json
```

The [oracle schema](schemas/nurbs-geometry-oracle.schema.json) pins both files'
SHA-256 and size, configuration, OCP version, explicit native-to-STEP bindings,
curve reversal, sampling density, and tolerances. STEP indices are one-based
unique edge/face indices in the pinned OCP import. Every selected native NURBS
carrier and STEP NURBS edge/face must be bound once. Ambiguous/missing bindings,
unsupported definitions, invalid hashes, and missing OCP fail the check.

The native evaluator uses homogeneous de Boor evaluation and analytical first
derivatives in Python. The STEP evaluator uses OCP's B-spline `D1`; shape
locations are applied by the BRep adaptors. See the official
[BRepAdaptor_Surface reference](https://occt3d.com/dev/doc/refman/html/class_b_rep_adaptor___surface.html)
and [Geom_BSplineSurface reference](https://occt3d.com/dev/doc/refman/html/class_geom___b_spline_surface.html).
The implementation is locally exercised with OCP 7.9.3.1; the online reference
may describe a newer release.

The report contains pole/knot/normalized-weight differences, sampled point
positions and first derivatives, per-sample errors, and fixed allowed errors.
It covers clamped, nonperiodic support curves/surfaces with positive weights and
C1 continuity at internal knots. Tensor poles use u-major, v-minor order.
The only orientation adjustment is the curve reversal specified in the oracle;
no best-fit matching is performed. Near-identical parameter endpoints may be
affinely mapped within the fixed knot tolerance, with derivative scaling.

This gate does not certify trim curves, pcurves, oriented face normals, areas,
volumes, watertightness, or a global geometric error bound. It does not change
parser provenance/exactness flags. OCP remains a validation-only dependency.

## Rectangular NURBS boundary diagnostics

`scripts/validate_nurbs_trim.py` extends a pinned, passing support oracle with
explicit face/coedge-to-STEP bindings. The
[trim oracle schema](schemas/nurbs-trim-oracle.schema.json) pins that support
oracle by hash and size and fixes a UV tolerance before comparison.

```sh
python scripts/validate_nurbs_trim.py trim-oracle.json fixture-root --output trim.json
```

This bounded diagnostic accepts one four-edge outer wire per selected NURBS
face, clamped nonperiodic support, and derived linear isoparametric pcurves.
Native curve intervals are derived from vertex positions by line projection or
unique full-support NURBS endpoint matching. Source ranges stay unchanged.
Present source ranges, alternate use curves, holes, seams, partial NURBS trims,
and other pcurve parameterizations are rejected until their semantics are
validated. STEP pcurves must be lines or nonrational two-pole degree-1 splines.

The report separates three gates:

- `derived_boundary_gate_passed`: curve/surface-lift positions, UV coordinates,
  first tangents per normalized traversal fraction, and closed rectangular
  boundaries agree at the fixed samples/tolerances.
- `oriented_trim_gate_passed`: directed edges, cyclic wire order, and face sense
  agree. Endpoint alignment for the geometry diagnostic is recorded and does
  not turn reversed traversal into an orientation pass.
- `source_trim_gate_passed`: remains false for this derived-interval profile;
  native source trim metadata is not certified by these measurements.

`comparison_completed=true` distinguishes completed measurements from invalid
inputs or missing dependencies. The CLI returns 1 unless all gates pass, so a
completed diagnostic can deliberately return 1. No parser flags are upgraded.

The reference pcurves come from OCP's imported STEP B-Rep and may be reconstructed
during transfer; they are not a SolidWorks API pcurve capture. Wire exploration
must cover every unique edge. See the official
[BRepTools_WireExplorer reference](https://occt3d.com/dev/doc/refman/html/class_b_rep_tools___wire_explorer.html)
and [BRepAdaptor_Curve2d reference](https://occt3d.com/dev/doc/refman/html/class_b_rep_adaptor___curve2d.html).
Local checks pin OCP 7.9.3.1; the online documentation may describe a newer version.
These finite samples do not establish a global error bound or oriented normals.

## Compatibility boundary

The `0.5.3+sldkit.4` backend includes native body/region/shell ownership for the
verified `SCH_3701229_37102_13006` layout. Native record links distinguish
solid and sheet bodies; unique `ConfigurationManager` IDs bind named
configurations to partition bodies even when XML order differs from the IDs.
A missing partition keeps membership unresolved. The unique saved most-recent
configuration ID identifies the active state when that source field is present.

The controlled SolidWorks fixture verifies Base with one solid and Derived
with two solids and one sheet, including separate body topology counts and
named membership. Other schemas, wire/general bodies, alternate record layouts,
and hierarchy edits in deltas remain outside this added profile. See the
[backend patch record](../vendor/cadmpeg-codec-sldprt/PATCHES.md).
Source location exactness does not imply complete byte-range coverage.

For that same verified native profile, FIN forward/backward links and forward
(end) vertices are converted to the graph builder's next/previous and start
vertex convention. Reciprocal FIN pairs, loop links, senses, and vertex joins
must agree before conversion. This fixes reversed boundary traversal without
using a STEP export or geometric matching to choose orientation. Invalid or
unsupported FIN graphs are withheld with `topology.native-fin-unresolved`;
their independently verified read domains remain in byte accounting.
The controlled NURBS boundary's four directed uses now agree with STEP.
Numeric source trim ranges and loop roles remain uncertified; the separate
source trim gate stays false.

The current profile decodes only modern Part containers. Assemblies, drawings,
legacy OLE2 geometry, feature-history reconstruction, healing, native writing,
and downstream common-IR conversion are outside this API. Geometry results can
be `partial` even when topology is useful—for example, when body hierarchy is
derived, tessellation face ownership is unresolved, or a carrier remains
untyped.

Saved DisplayLists meshes are also transferred when B-Rep decoding fails or
no Parasolid body stream is present. Such results remain `partial`, with
`fidelity.geometry_transferred=false` (this flag describes B-Rep transfer).
`geometry.not_transferred` retains the B-Rep diagnostic, while
`geometry.display_cache_transferred` reports that saved meshes are available.
Unresolved native FIN diagnostics are retained even when no B-Rep survives.
Mesh vertices, triangles, normals, native channels, appearance bindings, and
source provenance use the same reader as the B-Rep success path. Body/face
references and configuration ownership remain unresolved when B-Rep is absent.
