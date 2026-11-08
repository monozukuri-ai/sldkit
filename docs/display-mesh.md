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
    for instance in result.mesh.instances:  # FaceTessellations assemblies only
        print(instance.path, instance.tables_from)
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
| Modern Assembly (`Contents/DisplayLists`) | Meshes in component-local coordinates; `display.assembly_placement_unverified` notes that component placement is not decoded or applied |
| Modern Assembly (`FaceTessellations/*`, SOLIDWORKS 2023 and later) | One section of meshes per referenced part and the occurrence list `instances` read from the directory; the sections save no colors; see [FaceTessellations assemblies](#facetessellations-assemblies) |
| OLE2 Part or Assembly with `DisplayLists__ZLB` | Same table grammar after zlib decoding. A stream above 1 MiB holds several zlib members behind one magic, each with its own size header (SOLIDWORKS 2011 and 2012 assemblies); all members are decoded |
| OLE2 with `DisplayLists__Zip` | `display.stream_unreadable`; the older encoding is not decoded |
| Drawing | `unsupported` with `display.document_kind_unsupported` |
| No non-empty display stream | `unsupported` with `display.no_display_data` |

`decoded` means every non-empty display stream yielded validated tables.
`partial` keeps the recovered meshes when a stream is unreadable, several
`DisplayLists` streams hold meshes (`display.multiple_streams`; configuration
ownership is not resolved), a `FaceTessellations` directory, section, or
reference does not resolve, or the vertex budget stops packing
(`display.vertex_budget_exceeded`). The budget is
`max_total_uncompressed_bytes / 64` vertices under the selected limits.

Assembly display tables are returned in their stored coordinates, and those
coordinates are component-local: the faces of each instance are saved in the
frame of the referenced part document, not where the assembly places them. In
the manifold assembly recorded under
[observed evidence](development/assembly-validation.md#observed-evidence) the
stored tables span 46.5 x 89.7 x 210.0 mm with the four valve bodies
overlapping at the origin, while the placed assembly measures
356.8 x 168.0 x 138.5 mm. An earlier revision of this page described the
stored tables as the placed assembly; that held only for a sample whose
component transforms were small.

The stream carries the join that placement needs. The `DisplayLists` stream of
an Assembly starts with a `uoTempAssemblySHDData_c` declaration followed by an
MFC string (`ff fe ff`, length, UTF-16LE) holding the assembly's own name, and
the face tables of each instance are preceded by a `uoTempPartTessData_c`
record that begins with the instance path `<name>-<number>@<assembly>`; a
nested occurrence would read `<leaf>-<n>@<sub>-<m>@<root>`, leaf first. A face
table belongs to the nearest preceding record, which
`DisplayMeshPart.table_offset` locates in the decoded stream returned by
`extract_file(..., mode="decoded")`, and `<name>-<number>` matches
`AssemblyComponent.instance_name` and `instance_number`. sldkit does not
decode this join or apply `raw_transform`, so
`display.assembly_placement_unverified` stays and callers must not treat the
tables as component occurrences. In the manifold the stream holds tables for
one instance of each of its twelve unsuppressed part documents, so even a
placed display mesh covers 12 of the 24 occurrences in the vendor's STEP
export.

## FaceTessellations assemblies

Assemblies saved by SOLIDWORKS 2023 and later (internal versions 16000 and
up) keep `Contents/DisplayLists` for the environment only: line fonts, lights,
and the sub-assembly display header, but no face tables. The part display
tables live in `FaceTessellations/<stream>` entries (`000-000-005` and the
like) that `FaceTessellations/Directory` lists. Both are ordinary deflate
entries of the modern container.

The directory starts with a 16-byte header (`u32` format, `u32` last stream
number, `u32` record count, `u32` tree index of the first record) and holds one
record per component occurrence: the occurrence path as an MFC string
(`<name>-<n>@<assembly>`; a nested occurrence reads
`<sub>-<m>@<assembly>/<name>-<n>@<sub>`, root first), a `u32` model stamp, two
zero words, the 8-byte id of the owner this occurrence repeats (zero for an
owner), the record's own 8-byte id, the stream name as an MFC string (empty
when the occurrence owns no section), the `u32` section ordinal
(`0xffff_ffff` without one), the `u32` body count, 12 bytes per body, and the
`u32` tree index of the next record. Sub-assembly nodes consume a tree index
without a record; folders and pattern markers do get records.

A stream holds its sections in ordinal order. A section is the `u32` ordinal,
the owner's 8-byte id, the `u32` face count, the faces, and one trailing
`u32`. Each face is the `DisplayLists` face header (compact 8-byte or
extended 40-byte form), the same six-channel table that `DisplayLists`
uses, and a trailer with the table bounds as `f64`, flags, and the framed
`mo*SurfIdRep_c` references. Tables are not aligned, so sldkit tries every
strips descriptor and keeps a table only when a face header stating its
triangle and strip counts precedes it.

The result lists every `FaceTessellations/<stream>` after the `DisplayLists`
entries, packs each owner's tables once, and names the owner in
`DisplayMeshPart.instance_index`. `DisplayMeshDocument.instances` carries the
directory: `path`, the leaf `name` and `number`, `tree_index`, `tables_from`
(the instance whose tables the occurrence displays: itself for an owner, the
owner for a repeat, `None` when the file saved none, as for hidden,
suppressed, or lightweight components and for folders), `stream_index` and
`section_index` of an owned section, `declared_face_count` beside the
validated `face_count`, and `body_count`. The sections save no appearance, so
these parts have no color.

The tables are in the referenced part's own coordinates; a section's extent
equals the part model's `swBoundingBox` in `swXmlContents/COMPINSTANCETREE`.
A top-level occurrence joins `AssemblyComponent.instance_name` and
`instance_number` through the leaf `<name>-<n>`, and its `raw_transform` is
the placement. A nested occurrence needs the chain of `swReference`
transforms under the sub-assembly's `swModel` in `COMPINSTANCETREE`, composed
root first; sldkit exposes top-level components only and applies none of
these, so `display.assembly_placement_unverified` stays.

Diagnostics: `display.face_tessellations_directory` (info) summarizes the
occurrences. `display.face_tessellations_directory_unreadable`,
`display.face_tessellations_directory_incomplete`,
`display.face_tessellations_section_missing`,
`display.face_tessellations_section_incomplete`,
`display.face_tessellations_tables_unassigned`, and
`display.face_tessellations_reference_unresolved` are warnings that make the
result `partial` while the recovered tables are kept. The environment
`DisplayLists` stream is listed with `part_count` 0 and is not reported as
unreadable when the `FaceTessellations` streams yielded tables.

## Provenance

The face-table grammar, MFC class-tag reuse, persistent-surface references, and
appearance precedence are adopted from `cadmpeg-codec-sldprt` 0.5.3
(`tessellation.rs`, `appearance.rs`, Apache-2.0) and rewritten for sldkit's own
types in `crates/sldkit-parser/src/display/tables.rs` and `appearance.rs`, which
retain Apache-2.0. Container access, packing, outlines, derived normals, status,
and diagnostics are new sldkit code. The vendored decoder is not called. See
[third-party notices](../THIRD_PARTY_NOTICES.md).
