# Modern Part geometry

Geometry decoding is an explicit capability for modern chunk-form `.SLDPRT`
files. It is separate from metadata parsing so callers can inspect properties
and references without paying the geometry cost or accepting a geometry
dependency in their data flow.

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

## Compatibility boundary

The current profile decodes only modern Part containers. Assemblies, drawings,
legacy OLE2 geometry, feature-history reconstruction, healing, native writing,
and downstream common-IR conversion are outside this API. Geometry results can
be `partial` even when topology is useful—for example, when body hierarchy is
derived, tessellation face ownership is unresolved, or a carrier remains
untyped.
