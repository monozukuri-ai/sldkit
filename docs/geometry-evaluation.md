# Geometry evaluation

Geometry evaluation is a kernel-free derivation over an already decoded modern
Part geometry document. It samples edges, measures bounds, classifies edges by
the normals of their adjacent faces, and derives mesh quantities, so that a
downstream projection or drafting package can work from explicit polylines and
explicit withheld states instead of raw carriers. Nothing in the geometry
document is rewritten, and no geometry kernel is involved.

## API

```python
import sldkit

result = sldkit.evaluate_geometry_file("part.SLDPRT", chord_tolerance_mm=0.05)
if result.evaluation is not None:
    box = result.evaluation.bounding_box
    for edge in result.evaluation.sampled_edges():
        print(edge.edge_id, edge.curve_kind, edge.smoothness, len(edge.polyline))
    for mesh in result.evaluation.meshes:
        print(mesh.tessellation_id, mesh.is_closed, mesh.signed_volume_mm3)

decoded = sldkit.decode_geometry_file("part.SLDPRT")
again = sldkit.evaluate_geometry(decoded, configuration="Default")
```

`evaluate_geometry_bytes` and `evaluate_geometry_file` decode and evaluate in
one call; `evaluate_geometry` accepts a `GeometryResult` or `GeometryDocument`.
Options are `GeometryEvaluationOptions` or keyword overrides:
`chord_tolerance_mm` (0.01), `max_segments_per_edge` (512),
`tangent_angle_deg` (0.5), `mesh_feature_angle_deg` (30.0), and
`configuration`. `strict=True` accepts only `EVALUATED` and raises
`GeometryEvaluationError` with the structured result otherwise. The Rust API
provides `evaluate_geometry`, `evaluate_geometry_bytes`, and
`evaluate_geometry_path`; both command-line interfaces provide `evaluate`.

## Edges

Each B-Rep edge becomes an `EvaluatedEdge`:

- `interval_source` records whether the parameter interval is the decoder's
  `parameter_range` (`source`), the adapter's endpoint-derived interval
  (`derived`), or missing (`unavailable`). Unavailable intervals are never
  inferred here; the edge stays `unsampled` with a machine-readable `reason`.
- `polyline` is sampled within `chord_tolerance_mm` in millimetres; lines use
  two points, circles and ellipses uniform angular steps, and NURBS adaptive
  refinement bounded by `max_segments_per_edge`. The reported
  `chord_tolerance_mm` is the bound actually achieved.
- `smoothness` compares the outward normals of the two face uses at the
  parameter midpoint: `sharp` above `tangent_angle_deg`, `tangent` at or below
  it, `boundary` for a single face use, and `unknown` when a normal cannot be
  established, for example on procedural or unknown surface carriers.
  `dihedral_angle_deg` is unsigned.
- `is_derived_seam` marks edges the adapter synthesized to close periodic
  faces; they are not source edges.

## Bounds and meshes

`bounding_box`, `body_bounds`, and `configuration_bounds` combine sampled
polylines, analytic extrema of circles and ellipses, and tessellation vertices.
`exact` is `true` only when every contributing face is planar, every
contributing edge is a line, circle, or ellipse over a known interval, and no
body carries an unapplied transform. Configuration bounds stay absent when the
decoder left body membership unresolved.

Each display tessellation becomes an `EvaluatedMesh` with bounds, area,
closedness, a divergence-theorem signed volume for closed meshes only, feature
edges above `mesh_feature_angle_deg`, and boundary edges. Local fixtures store
per-face meshes, so they report `is_closed=False` and no volume; whole-body
meshes are covered by synthetic tests.

## Status

`evaluated` means every edge was sampled and classified. `partial` keeps the
useful subset and lists aggregated `losses` such as
`evaluation.edge_interval_unavailable`,
`evaluation.surface_normal_unavailable`, or
`evaluation.body_transform_not_applied`. `unavailable` means the input had no
geometry document, and `rejected` means invalid options or a configuration name
that does not exist. Stored body transforms are reported, not applied.
