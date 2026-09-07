# sldkit

`sldkit` is an experimental, source-faithful parser for SolidWorks `.SLDPRT`,
`.SLDASM`, and `.SLDDRW` files. The parsing core is written in Rust and exposed
as a typed Python package through PyO3.

## Capabilities

| Capability | Support |
|---|---|
| Container detection and bounded inventory | Modern chunk, OLE2/CFB, and ZIP/OPC candidates |
| Stored and decoded stream extraction | Supported where the container decoder recognizes the encoding |
| Exact binary-resource extraction | Revalidates parser-produced path, decoded range, and SHA-256 before returning preview bytes |
| Modern metadata, properties, configurations, and references | Partial, source-faithful profile |
| Modern `.SLDPRT` B-Rep and tessellation | Explicit partial profile with provenance and loss records |
| Modern `.SLDDRW` source structure | Exact XML record inventory plus unframed Drawing carrier candidates; no render semantics |
| Directory project graph | Bounded and deterministic for decoded references |
| Legacy OLE2/CFB metadata, properties, configurations, and previews | Partial, bounded profile for observed layouts |
| ZIP/OPC document semantics | Unsupported |
| Feature history, mates, and occurrence transforms | Unsupported |
| Drawing entities, dimensions, and view transforms | Unsupported |

The package returns structured diagnostics and byte coverage. Missing,
unsupported, malformed, and inferred source data are not collapsed into empty
values or successful parses.

See the [public documentation](https://github.com/monozukuri-ai/sldkit/tree/main/docs)
for architecture, compatibility boundaries, project scanning, and parser-rule
provenance.

## Python

```python
import sldkit

probe = sldkit.probe_file("part.SLDPRT")
print(probe.envelope, probe.confidence)

inventory = sldkit.inspect_file("part.SLDPRT")
for entry in inventory.inventory.entries if inventory.inventory else ():
    print(entry.id, entry.path, entry.checksum)

entry = inventory.inventory.entries[0]
extracted = sldkit.extract_file("part.SLDPRT", entry.id)
assert extracted.data is not None

result = sldkit.parse_file("part.SLDPRT")
for config in result.document.configurations if result.document else ():
    print(config.index.value, config.name.value if config.name else None)
for prop in result.document.properties if result.document else ():
    print(prop.name.value, prop.value_state, prop.raw_value)
for diagnostic in result.diagnostics:
    print(diagnostic.code, diagnostic.kind, diagnostic.message)

for sheet in result.document.sheets if result.document else ():
    if sheet.preview is not None:
        preview = sldkit.extract_resource_file("drawing.SLDDRW", sheet.preview)
        assert preview.data is not None

geometry = sldkit.decode_geometry_file("part.SLDPRT")
if geometry.geometry is not None:
    print(len(geometry.geometry.model.bodies))
    for metric in geometry.geometry.topology_metrics:
        print(metric.body_id, metric.faces, metric.edges, metric.vertices)
    for loss in geometry.geometry.fidelity.losses:
        print(loss.code, loss.category, loss.severity)

drawing = sldkit.decode_drawing_structure_file("drawing.SLDDRW")
if drawing.structure is not None:
    for record in drawing.structure.records:
        print(record.id, record.record_class, record.source.decoded_offset)
    for carrier in drawing.structure.source_streams:
        print(carrier.stream_path, carrier.record_framing_verified)

graph = sldkit.scan_project(
    "project/top.SLDASM",
    project_root="project",
    configuration="Default",
)
for edge in graph.edges:
    print(edge.stored_path, edge.resolution_status, edge.resolved_path)
```

Use `strict=True` with `parse_file` or `parse_bytes` when an unsupported or
partial result must raise `sldkit.ParseError`. Geometry decoding has the same
option and raises `sldkit.GeometryError` unless its status is `decoded`.
Drawing structure inventory raises `sldkit.DrawingStructureError` in strict
mode unless its status is `inventoried`; a `partial` result retains all located
records and exact candidate-stream identities.

If no configuration is selected, the graph is the union of all decoded source
configurations. Configuration names are matched exactly. Suppressed references
are resolved but not traversed unless `follow_suppressed=True` is set.

The full graph contains source paths and hashes. Use the path-free aggregate
when sharing compatibility results:

```bash
sldkit scan project/top.SLDASM --project-root project --summary
sldkit-rs scan project/top.SLDASM --project-root project --summary
sldkit drawing drawing.SLDDRW --limits service
sldkit-rs drawing drawing.SLDDRW --limits service
uv run python scripts/compare_drawing_structures.py baseline.SLDDRW variant.SLDDRW
```

Windows absolute paths are never opened as host paths on Linux or macOS. An
explicit relocation can be supplied with
`--windows-prefix-map 'Z:\\CAD=/mnt/cad'`; unresolved basename fallback remains
labeled and never selects among multiple candidates.

Geometry fidelity includes verified per-domain field spans and typed/uninterpreted
byte ranges. Complete byte accounting is separate from semantic geometry coverage;
see [geometry fidelity](docs/geometry.md).

## Architecture boundary

`sldkit-parser` uses the published Rust `parasolid-core` crate for embedded
Parasolid headers and shared partial topology/geometry readers. No Python `parasolid-kit` installation or adjacent
checkout is required. See the [migration boundary](docs/geometry.md#parasolid-dependency).

`sldkit` owns SolidWorks-specific parsing and source models. It does not depend
on `cad3d-ir`, CadQuery, Open CASCADE, a vendor SDK, COM, or a viewer. A separate
adapter can depend on both `sldkit` and a downstream interchange model.
The source distribution's optional SolidWorks capture script is controlled
validation tooling; it is not imported by the package or included in wheels.

## Development

```bash
cargo test --workspace
CARGO_TARGET_DIR=target cargo test --locked --manifest-path vendor/cadmpeg-codec-sldprt/Cargo.toml --lib
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo +nightly fuzz run inventory -- -runs=10000
uv sync --dev
uv run maturin develop
uv run pytest
uv run ruff check .
```

The cross-platform wheel gate builds one CPython 3.10+ ABI3 wheel per configured
OS, inspects its contents, and installs it without dependencies or an index on
both Python 3.10 and 3.14 before running semantic parsing, exact-resource
extraction, and project-graph smoke checks. A separate job rebuilds a Linux wheel
from the source distribution and applies the same checks.

```bash
uv run --frozen maturin build --release --locked --out dist
uv run --frozen python scripts/verify_release_artifacts.py dist
uv run --frozen python scripts/smoke_wheel_artifact.py dist
```

## License

MIT. External validation inputs are not part of the installed package or source
distribution.
