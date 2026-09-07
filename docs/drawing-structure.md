# Modern Drawing structure

Drawing structure inventory is an explicit capability separate from metadata
parsing and Part geometry decoding. It preserves source record identity and
hierarchy without claiming that a `.SLDDRW` can yet be rendered or converted to
DXF, SVG, or PDF.

## API

Python exposes `decode_drawing_structure_file` and
`decode_drawing_structure_bytes`:

```python
import sldkit

result = sldkit.decode_drawing_structure_file(
    "drawing.SLDDRW",
    profile="service",
)
if result.structure is not None:
    for record in result.structure.records:
        print(
            record.id,
            record.record_class,
            record.parent_id,
            record.source.stream_path,
            record.source.decoded_offset,
            record.source.byte_len,
        )
```

The Rust API provides `decode_drawing_structure_path` and
`decode_drawing_structure_bytes`. Both command-line interfaces provide a
`drawing` command:

```bash
sldkit drawing drawing.SLDDRW --limits service
sldkit-rs drawing drawing.SLDDRW --limits service
```

`strict=True` accepts only `inventoried` and raises
`DrawingStructureError` for `partial`, `unsupported`, `malformed`, or
`rejected` while retaining the structured result on the exception. The current
M6a slice always reports `partial` because its exclusive byte-partition gate is
not yet complete; `inventoried` is reserved for that future gate.

## Record inventory

Each safely decoded `swXmlContents/KeyWords` XML element becomes one
`DrawingRecord`. The record retains:

- a deterministic inventory ID;
- the XML local element name, parsed source attributes, and trimmed direct text;
- its parent record ID;
- its exact decoded-stream offset, byte length, and SHA-256;
- a class derived only from the XML local element name.

The ID is stable for identical source bytes and container entry identity. It is
not a SolidWorks persistent reference and is not promised to survive an edited
save. A class such as `note`, `sketch`, or `view` is an inventory index, not
evidence that its render semantics have been decoded.

Attribute ordering, quoting, namespace prefixes, and untrimmed text are not
reconstructed from the parsed convenience fields. They remain recoverable from
the exact decoded range identified by `entry_id`, offset, length, and SHA-256.

Direct XML membership currently supports sheet identity and direct child views,
including the referenced document and configuration fields already present in
the source XML. Sheet-format records and views outside a sheet remain in the
generic record inventory. View dependencies are left unset until controlled
fixtures and an independent SolidWorks API capture establish them.

## Candidate carriers and coverage

Exact decoded identities for `Contents/Definition`,
`Contents/DisplayLists`, and `Contents/VBLists` are retained as candidate
carriers. Their role is path-derived, `record_framing_verified` is `false`, and
no dimensions, annotations, geometry, transforms, or projection are inferred
from their names or payloads.

Until record framing is proven, coverage reports:

- `partition_status = incomplete`;
- `typed_bytes = null`;
- `uninterpreted_bytes = null`.

This avoids treating overlapping XML element ranges or whole candidate streams
as an exclusive byte partition. A `partial` result is useful for investigation
and deterministic comparison, but it is not renderable Drawing support.

## Current boundary

The repository now provides a path-free SolidWorks API capture contract and a
deterministic comparator for controlled single-variable Drawing pairs. See
[Controlled Drawing validation](drawing-validation.md). No Windows/SolidWorks
capture or controlled native fixture is committed by this tooling change, so
M6a still requires measured cases for multiple sheets, dependent view types,
dimensions, annotations, tables, and sketches. Binary carrier record framing
and an exclusive classified/unclassified byte map must also be established
before the M6a completion gate can pass. M6b will separately add verified
placement, projection, and renderable primitives while retaining these source
identities.
