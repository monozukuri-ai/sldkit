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
5. **Scan** resolves decoded document references within explicit filesystem
   roots and produces a dependency graph.

Each stage has its own result status. A recognized container does not imply that
all document semantics were decoded, and a complete reference graph does not
imply complete geometry or feature coverage.

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
