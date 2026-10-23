# Compatibility

`sldkit` exposes only capabilities that can report partial coverage and
unsupported data explicitly. A recognized file is not necessarily fully
parsed.

## Supported capabilities

| Area | Behavior |
|---|---|
| Format probing | Detects modern chunk, OLE2/CFB, ZIP/OPC, malformed, and unknown candidates from content |
| Container inventory | Bounded stream and storage inventory for recognized layouts |
| Stream extraction | Stored or decoded entry bytes, plus exact parser-produced binary resources with path/range/SHA-256 revalidation |
| Modern documents | Partial decoding of document kind, properties, configurations, cached metadata, assembly references, drawing sheets, and drawing-view references |
| Typed document facts | Component flags and raw transform tokens, saved bounding boxes, material labels, unit settings, feature-record inventory, sheet size/scale/format facts, and embedded virtual-component documents; see [document facts](document-facts.md) |
| Modern Part geometry | Explicit partial decoding of B-Rep topology, analytic and NURBS carriers, display tessellation, configuration body state, provenance, and loss records |
| Modern Drawing structure | Deterministic inventory of `KeyWords` XML elements with exact decoded ranges, hierarchy, parsed source fields, direct sheet/view membership, and exact unframed carrier identities |
| Legacy OLE2/CFB documents | Partial decoding of document kind, internal version, standard and application property sets, selected configuration headers, and DIB or PNG previews |
| Saved display meshes | Packed `DisplayLists` meshes, saved colors, and face outlines for modern Parts and Assemblies and `__ZLB` OLE2 Parts, independent of B-Rep decoding; see [display meshes](display-mesh.md) |
| Geometry evaluation | Kernel-free sampled edge polylines, bounding boxes, edge smoothness from adjacent face normals, and mesh quantities over decoded modern Part geometry |
| Project scanning | Bounded dependency graph for decoded Part, Assembly, and Drawing references, including virtual components embedded in their assembly |
| Project index and bill of materials | Whole-project reverse-reference graph and occurrence expansion with per-document, per-configuration quantities |
| Python API | Typed models corresponding to the Rust JSON contract |
| Command-line interface | Probe, inspect, extract, parse, geometry, evaluate, Drawing structure, project scan, project index, and bill-of-materials operations |
| Python HTML viewer | Recovered triangle meshes with camera and visibility controls, bounded saved PNG/DIB previews, and parse diagnostics; no geometry reconstruction |

Modern parsing is limited to observed layouts. Internal version values are
evidence attached to an input, not a promise that every file from a product year
or every intermediate version is supported.

The controlled Assembly and Drawing capture/differential scripts are validation tools,
not runtime format support. Their output can establish external fixture facts;
it does not make dimensions, view transforms, or candidate binary streams
decoded capabilities.

The bounded edge API also exposes independently labelled endpoint-derived
parameter intervals for lines and eligible full-support NURBS curves; see
[geometry.md](geometry.md#endpoint-derived-edge-intervals). This does not certify
source trim ranges or inner/outer loop roles. Assembly placement evidence can be
collected with the [controlled capture workflow](development/assembly-validation.md).

Legacy property-set and preview decoding is container- and specification-based.
Configuration-header decoding is currently limited to the observed internal
versions `2200` and `7000`, with one configuration record in the supported
layout. Other streams remain inventory entries and unknown records. A generic
compound file without SolidWorks-specific content evidence is not reported as a
partially parsed SolidWorks document.

## Unsupported capabilities

- Legacy feature data, model references, assembly occurrences, drawing sheets,
  and geometry
- Semantic decoding of ZIP/OPC SolidWorks content
- Legacy, Assembly, and Drawing B-Rep geometry, and legacy `DisplayLists__Zip`
  display streams; saved display meshes of modern Parts and Assemblies and of
  `__ZLB` OLE2 Parts are covered by [display meshes](display-mesh.md)
- Feature-history reconstruction; the saved feature-record inventory is a
  name and display-value index only
- Mate semantics and component occurrence transforms; the sixteen `swTransform`
  tokens are retained as unverified evidence
- Material density and mass derivation; material labels are not resolved
  against a material library
- Renderable Drawing entities, dimensions, annotations, projection, and view transforms
- Native file writing or round-trip editing

Unsupported content remains visible through diagnostics, inventory entries,
unknown records, and semantic coverage whenever the enclosing structure can be
read safely.

## Result status

`ParseStatus` describes semantic coverage for one document. `partial` means
useful source facts were decoded but unsupported or uninterpreted content
remains.

`GeometryStatus` independently describes modern Part geometry. `partial` can
contain useful B-Rep and tessellation while the fidelity report identifies
derived topology, unresolved ownership, untyped carriers, or incomplete byte
partitioning.

`DrawingStructureStatus` independently describes modern Drawing record
inventory. `partial` can contain exact XML record ranges and carrier hashes
while binary record framing, typed fields, and exclusive byte partitioning
remain unverified. Stable inventory IDs are not persistent references across a
modified save.

`GeometryEvaluationStatus` describes the kernel-free evaluation of decoded
geometry. `partial` keeps sampled edges and bounds while listing withheld edges,
normals, or meshes as losses.

`BomStatus` describes occurrence expansion over a scanned graph. `partial`
means at least one occurrence could not be expanded or resolved; `rejected`
means the root configuration was ambiguous or the graph itself was rejected.

`ProjectScanStatus` describes reference resolution for a project. `complete`
means every decoded reference required by the selected traversal was resolved
under the configured rules. It does not upgrade the parse status of any node.

Applications that require complete semantics should reject partial results or
use the Python API's strict mode.
