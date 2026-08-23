# Compatibility

`sldkit` exposes only capabilities that can report partial coverage and
unsupported data explicitly. A recognized file is not necessarily fully
parsed.

## Supported capabilities

| Area | Behavior |
|---|---|
| Format probing | Detects modern chunk, OLE2/CFB, ZIP/OPC, malformed, and unknown candidates from content |
| Container inventory | Bounded stream and storage inventory for recognized layouts |
| Stream extraction | Stored bytes and decoded bytes where the compression method is supported |
| Modern documents | Partial decoding of document kind, properties, configurations, cached metadata, assembly references, drawing sheets, and drawing-view references |
| Modern Part geometry | Explicit partial decoding of B-Rep topology, analytic and NURBS carriers, display tessellation, configuration body state, provenance, and loss records |
| Legacy OLE2/CFB documents | Partial decoding of document kind, internal version, standard and application property sets, selected configuration headers, and DIB or PNG previews |
| Project scanning | Bounded dependency graph for decoded Part, Assembly, and Drawing references |
| Python API | Typed models corresponding to the Rust JSON contract |
| Command-line interface | Probe, inspect, extract, parse, geometry, and project scan operations |

Modern parsing is limited to observed layouts. Internal version values are
evidence attached to an input, not a promise that every file from a product year
or every intermediate version is supported.

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
- Legacy, Assembly, and Drawing B-Rep or tessellation geometry
- Feature-history reconstruction
- Mate semantics and component occurrence transforms
- Drawing entities, dimensions, annotations, and view transforms
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

`ProjectScanStatus` describes reference resolution for a project. `complete`
means every decoded reference required by the selected traversal was resolved
under the configured rules. It does not upgrade the parse status of any node.

Applications that require complete semantics should reject partial results or
use the Python API's strict mode.
