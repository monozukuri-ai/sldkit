# Saved display meshes

`decode_display_mesh_*` returns the triangle meshes that SolidWorks saved in
`DisplayLists` streams, without running the B-Rep decoder. It is the fast and
bounded path for viewers and mesh export. B-Rep failures, byte-ledger findings,
and B-Rep cost do not affect it.

```python
import array
import sldkit

result = sldkit.decode_display_mesh_file("part.SLDPRT", profile="service")
if result.mesh is not None:
    positions = array.array("f", result.positions)  # x, y, z per vertex, millimetres
    normals = array.array("f", result.normals)      # x, y, z per vertex
    triangles = array.array("I", result.indices)    # three vertex indices each
    edges = array.array("I", result.edges)          # two vertex indices each
    for part in result.mesh.parts:
        print(part.id, part.triangle_count, part.color, part.color_source)
```

```bash
sldkit display part.SLDPRT --limits service
```

The CLI prints the JSON contract only. Rust callers use
`sldkit_parser::decode_display_mesh_bytes` and `decode_display_mesh_path`, which
return the result and a `DisplayMeshBuffers` value.

## Contract

The JSON result describes streams and parts. The packed arrays travel beside it
and are never expanded into JSON or Python objects:

| Buffer | Layout |
|---|---|
| `positions` | little-endian `f32` x, y, z per vertex, in millimetres |
| `normals` | little-endian `f32` x, y, z per vertex |
| `indices` | little-endian `u32`, three per triangle |
| `edges` | little-endian `u32`, two per face-outline segment |

Indices address the shared vertex arrays. Each part records its
`vertex_start`/`vertex_count`, `triangle_start`/`triangle_count`, and
`edge_start`/`edge_count` slices in part order.

- **Coordinates** are the stored `f32` metres scaled once to millimetres. They
  keep SolidWorks axis directions; no unit or axis conversion is repeated.
- **Triangles** come from the stored strips with alternating winding, identical
  to the tessellations of `decode_geometry_*`.
- **Normals** are the stored per-vertex normals. A table without normals gets
  area-weighted vertex normals and `normals_derived=true`.
- **Edges** are face outlines: within one table, coincident positions are welded
  by their exact `f32` bits and each edge used by exactly one triangle is kept.
  Display tables are face-local, so these are the visible CAD face boundaries,
  including tangent ones. They are not a feature-angle classification.
- **Colors** follow the saved precedence: body (`uoBodyPropInfo_c`), then
  feature (`ThirdPtyStore/VisualStates`), then face-local inline properties.
  `color_source` names the level; a part without saved appearance has no color.
- **Surface references** are the framed `mo*SurfIdRep_c` identities saved after
  a table. They are source identities, not B-Rep face ownership.

## Coverage

| Input | Behavior |
|---|---|
| Modern Part (`Contents/DisplayLists`) | Meshes, colors, and outlines |
| Modern Assembly | Meshes as stored; `display.assembly_placement_unverified` notes that component placement is not decoded |
| OLE2 Part with `DisplayLists__ZLB` | Same table grammar after zlib decoding |
| OLE2 with `DisplayLists__Zip` | `display.stream_unreadable`; the older encoding is not decoded |
| Drawing | `unsupported` with `display.document_kind_unsupported` |
| No non-empty display stream | `unsupported` with `display.no_display_data` |

`decoded` means every non-empty display stream yielded validated tables.
`partial` keeps the recovered meshes when a stream is unreadable, several
streams hold meshes (`display.multiple_streams`; configuration ownership is not
resolved), or the vertex budget stops packing
(`display.vertex_budget_exceeded`). The budget is
`max_total_uncompressed_bytes / 64` vertices under the selected limits.

Assembly display tables are returned in their stored coordinates. In the
observed files they show the placed assembly, but placement is not decoded or
verified, so callers must not treat them as component occurrences.

## Provenance

The face-table grammar, MFC class-tag reuse, persistent-surface references, and
appearance precedence are adopted from `cadmpeg-codec-sldprt` 0.5.3
(`tessellation.rs`, `appearance.rs`, Apache-2.0) and rewritten for sldkit's own
types in `crates/sldkit-parser/src/display/tables.rs` and `appearance.rs`, which
retain Apache-2.0. Container access, packing, outlines, derived normals, status,
and diagnostics are new sldkit code. The vendored decoder is not called. See
[third-party notices](../THIRD_PARTY_NOTICES.md).
