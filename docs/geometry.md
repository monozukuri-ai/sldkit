# Modern Part geometry

Geometry decoding is an explicit capability for modern chunk-form `.SLDPRT`
files. It is separate from metadata parsing so callers can inspect properties
and references without paying the geometry cost or accepting a geometry
dependency in their data flow.

## Parasolid dependency

`sldkit-parser` and its SolidWorks adapter use the published
`parasolid-core = "=0.3.2"` Rust crate. No Python `parasolid-kit` package or
adjacent checkout is required. Standard embedded `X_B` headers are validated
under the caller's size/string limits. The public `parasolid_body` origin remains
immediately after the schema string; offsets and hashes retain their meaning.
The two source-less cadmpeg writer keys retain their explicit local header reader.

The production path calls `parasolid_core::partial` for compact topology,
validated BODY/REGION/SHELL ownership, FIN normalization, analytic/NURBS carriers,
intersection caches, sweep/spin, offset, blend, subset records, and exact read
spans where instrumented. Point and NURBS patch helpers are shared too. Core
values stay in source units; the adapter converts
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
[partial-reader contract and provenance](https://docs.rs/crate/parasolid-core/0.3.2/source/PARTIAL_READERS.md).

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

Within the verified native hierarchy/FIN profile, a sheet edge can take its
stored direction from a boundary dummy FIN outside every face loop. The
adapter requires reciprocal links and agreement with the visible loop's two
endpoints before emitting that edge. The dummy does not become a public coedge;
the visible coedge retains its direction relative to the edge.

The disk-face `V-E+F` census is withheld (`euler_characteristic=None`) when a
face has multiple loops, no loop, or isolated vertex uses. In particular, a
face with holes must not produce a misleading Euler value from that formula.
Even a reported value is a topology diagnostic, not solid-validity certification.

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

### Endpoint-derived edge intervals

`GeometryEdge.parameter_range` retains the decoder's range unchanged. When it
is absent, the adapter may additionally expose `derived_parameter_interval`:

- `parameter_range`: directed from the edge's start vertex to its end vertex;
- `method`: `line_projection`, `nurbs_support_endpoints`, `nurbs_monotone_projection`, `conic_endpoints`,
  or `closed_circle_seam`;
- `tolerance_mm` and `max_endpoint_error_mm`: the endpoint verification evidence.

Line projection supports non-unit directions. The NURBS profile requires a
clamped nonperiodic curve with finite data, positive weights where present,
and unambiguous correspondence of the vertices to the full support endpoints.
Partial NURBS intervals additionally require one strictly monotone coordinate
of the complete control polygon. Positive weights and continuous, clamped
knots establish a unique inverse on that coordinate. Bisection is followed by
3D endpoint checks. This bounded method supports degree 1–16 and at most 1024
control points; ambiguous, discontinuous and numerically unsafe cases fail
closed. Parameter order follows vertex order, including reversed intervals.
Conic intervals require the verified native hierarchy/FIN profile. Open
circles and ellipses follow the positive native carrier direction, including
arcs longer than pi and intervals crossing the 2-pi branch. A full circle
requires an explicit `derived_closed_circle_seam` vertex shared by both ends;
coincident endpoints alone do not establish a full turn. Closed ellipses are
outside this profile. All methods require endpoint errors at most `1e-7` mm.
Off-curve, ambiguous and other unsupported cases keep
this field absent. This does not infer stored numeric trim metadata or surface
UV bounds.

Python's `edge.effective_parameter_range` returns the existing decoded range
first, otherwise this derived interval, otherwise `None`. Inspect
`derived_parameter_interval` when the distinction matters. Coedge reversal
reverses traversal; it does not change the edge's endpoint ordering. The new
field is optional in JSON and older JSON remains readable.

### Derived analytic loop roles

`GeometryLoop.boundary_role` retains its source classification. The optional
`derived_boundary_role` contains `role` (`outer` or `inner`),
`method` (`planar_analytic_winding` or `cylindrical_analytic_chart`),
`signed_area_mm2`, and `tolerance_mm`.
Python's `effective_boundary_role` prefers a specified source role, then the
derived role, and otherwise returns `unspecified`.

Derivation requires a verified native plane face with simple closed line and
circular-arc boundaries and matching derived planar pcurves. Analytic area
integrals and line/arc intersections establish one outer ring and disjoint,
contained holes; list order is irrelevant. Area signs include FACE sense:
outer positive, holes negative. Broken links, intersections, touching or
nested holes, alternate use curves, ambiguous supports and unsupported
carriers withhold the entire face's classification. Work is bounded to 128
loops and 512 total coedges per face. Closure tolerance is `1e-7` mm.

The cylindrical method accepts one simple outer loop and disjoint, contained
holes made of axial lines and circular/elliptical cylinder sections. Continuous
angular unwrapping, analytic intersections, containment and oriented area
establish the roles independently of loop order. Each loop has 4–128 coedges;
the face is limited to 128 loops and 512 total coedges. A full-period outer
chart additionally requires an explicit pair of opposite derived seam uses.
The current seam builder handles the two native ring loops of a full cylinder;
it does not yet construct that chart for a full cylinder with additional holes.
Each hole must fit strictly inside one unambiguous angular copy of the outer
chart. Holes crossing the chart's cut, touching or nested loops, unsupported
pcurve parameterizations and other periodic surfaces remain unspecified;
one unsupported loop withholds classification for the whole face.
Multiple-loop cylinder classification has synthetic tests and an independent
OCP area reference; saved/reopened SolidWorks qualification remains pending.
When a seam is present, classification describes the public seam-cut loop;
the two native ring loops of a full cylinder are retained in provenance.
The cylindrical axial line pcurve uses the 3D carrier's parameter-zero origin and axial rate, so
`surface(pcurve(t)) == curve(t)` throughout the trimmed interval. Added seam
topology and modified loop membership retain derived field provenance.
These pcurves and loop roles are geometric derivations, not recovered stored
trim fields. Partial NURBS interval derivation is covered by synthetic tests;
independent qualification with a saved/reopened SolidWorks Part remains pending.
See [geometry validation](development/geometry-validation.md) and
[Part boundary validation](development/part-validation.md) for reusable
validation tools and their limits.

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


## Compatibility boundary

The `0.5.3+sldkit.4` backend includes native body/region/shell ownership for the
verified `SCH_3701229_37102_13006` layout. Native record links distinguish
solid and sheet bodies; unique `ConfigurationManager` IDs bind named
configurations to partition bodies even when XML order differs from the IDs.
A missing partition keeps membership unresolved. The unique saved most-recent
configuration ID identifies the active state when that source field is present.

Other schemas, wire/general bodies, alternate record layouts,
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
Numeric source trim ranges and stored loop-role fields remain uncertified;
the separate source trim gate stays false. Derived planar roles have their own
bounded contract and API validation described above.

Vertexless ring edges are additionally supported for exact unbounded circle
carriers in this same native profile. Their source null vertices remain null
during FIN validation. The graph adapter emits explicitly derived circle seam
vertices and periodic seam edges where required; it does not claim these are
native vertex/edge records. Elliptic/NURBS rings and unsupported FIN links remain
outside this extension. See the patch record for the complete bounded checks.

Bodies stored only in `FeatureBodies/LocalBodies` are not currently restored
from that storage.

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

Display-face persistent references can constrain the join to emitted Parasolid
`ATOM_ID_2001` face identities. A display stream is scoped to a configuration
only when all its tables have consistent references, complete body memberships
select exactly one configuration, and its name is independently present in the
stream tail. A saved active flag or name alone never selects ownership.
Analytic carrier/trim checks disambiguate remaining candidates; a unique source
reference can also identify a NURBS face. Conflicts and non-unique joins remain
unresolved. These body/face bindings are marked `Derived` in the decoder's field
annotations and reported by `geometry.tessellation_ownership_derived`; they do
not certify direct native owner pointers or overall model completeness.

The bounded DisplayLists reader also accepts reused MFC face-class tags learned
from validated instances inside the first declared face interval. Each added
table must satisfy the same header/channel checks, and already read tables are
not duplicated. This covers continuation after body-property records without
claiming a general MFC archive reader.
