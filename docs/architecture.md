# Architecture

## Scope

`sldkit` parses SolidWorks-specific containers and document metadata. It keeps
source semantics separate from downstream CAD interchange models, geometry
kernels, viewers, and vendor automation APIs.

The package has two public implementation layers:

1. A Rust core performs bounded container inspection, stream extraction,
   semantic decoding, diagnostics, and project-reference resolution.
2. A PyO3 extension exposes the Rust results to typed Python models and a Python
   command-line interface.

No vendor installation is required at runtime.

The opt-in `sldkit.viewer` module is a downstream consumer of these public Python
results. It writes standalone HTML with bundled Three.js assets; the parser and
package-root API do not import the viewer. No Python runtime dependencies or
geometry kernel are added. See [offline viewer](viewer.md).

The parser depends on the published `parasolid-core` Rust crate for embedded
headers and shared partial Parasolid record readers. The SolidWorks adapter
selects streams/configurations, converts source units to public models, and
retains diagnostics and source provenance. Full schema parsing and the bounded
partial reader API have separate support contracts; see the
[current boundary](geometry.md#parasolid-dependency).

The source distribution also carries an optional Windows PowerShell script for
controlled SolidWorks API validation. It is not imported by either runtime
layer, is not included in wheels, and does not provide a parser fallback.

## Data flow

An input follows this sequence:

1. **Probe** identifies a supported container candidate from bytes rather than
   trusting the filename extension.
2. **Inspect** inventories bounded streams and records checksums, compression,
   byte ranges, and undecoded regions.
3. **Extract** returns a selected stored or decoded stream when its encoding is
   supported.
4. **Parse** converts recognized document metadata into source-faithful values
   with evidence and diagnostics.
5. **Drawing** inventories modern Drawing source records and exact carrier
   identities without assigning renderable semantics.
6. **Geometry** explicitly decodes modern Part topology, carriers, and
   tessellation with provenance and a loss report.
7. **Scan** resolves decoded document references within explicit filesystem
   roots and produces a dependency graph.

Each stage has its own result status. A recognized container does not imply that
all document semantics were decoded. Geometry success is reported separately,
Drawing record inventory is reported separately, and a complete reference graph
does not imply complete geometry or feature coverage.

Modern and legacy inputs share the same source model, but only when the source
provides the corresponding fact. For example, a legacy file may provide core
properties and a preview while references and geometry remain absent and
explicitly uninterpreted. Values unavailable in one envelope are not populated
from defaults observed in another envelope.

## Source-faithful values

Parsed values retain their source location and confidence where available.
Unknown, missing, empty, malformed, and unsupported values remain distinct.
The parser does not synthesize geometry or silently replace unknown booleans
with `false`.

Diagnostics use stable machine-readable codes. Coverage records distinguish
fully interpreted, partially interpreted, uninterpreted, and malformed streams
and bytes.

## Resource boundaries

All public byte and path entry points use explicit limits for input size,
decoded size, stream count, string length, compression ratio, XML size, graph
size, and traversal depth. Limit failures are reported as structured rejected
results rather than unbounded allocation or recursion.

## Downstream integration

The source model is the public output boundary. Conversion to a shared CAD IR
belongs in a separate adapter so that source uncertainty, unsupported fields,
and format-specific evidence are not lost inside the parser.

The modern Part geometry model follows the same boundary. It exposes
source-oriented topology, carrier parameters, tessellation, exact stream
identity, and loss records; it does not expose a downstream kernel handle or a
shared interchange model. See [Modern Part geometry](geometry.md).

The modern Drawing structure model similarly keeps exact source-record ranges,
stable inventory IDs, and unframed carrier candidates separate from any 2D IR.
It does not claim to reconstruct dimensions, annotations, projection, or view
placement. See [Modern Drawing structure](drawing-structure.md).
