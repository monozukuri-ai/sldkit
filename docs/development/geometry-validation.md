# Geometry validation

These tools compare parser output with independently captured reference data.
Run the commands from a repository checkout or extracted source distribution
with the development dependencies installed. OCP is an optional validation
dependency; it is not required to use sldkit.

See [Part geometry](../geometry.md) for the public API and support limits.

## Independent geometry validation

Neutral B-Rep reference values can be recorded against
[`schemas/geometry-oracle.schema.json`](../schemas/geometry-oracle.schema.json).
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

## Numerical NURBS comparison

`scripts/validate_nurbs_geometry.py` compares the native NURBS support carriers
referenced by a named configuration against STEP B-splines. Run it in a
validation environment with OCP and the repository's Python development tools:

```sh
python scripts/validate_nurbs_geometry.py oracle.json fixture-root --output result.json
```

The [oracle schema](../schemas/nurbs-geometry-oracle.schema.json) pins both files'
SHA-256 and size, configuration, OCP version, explicit native-to-STEP bindings,
curve reversal, sampling density, and tolerances. STEP indices are one-based
unique edge/face indices in the pinned OCP import. Every selected native NURBS
carrier and STEP NURBS edge/face must be bound once. Ambiguous/missing bindings,
unsupported definitions, invalid hashes, and missing OCP fail the check.

The native evaluator uses homogeneous de Boor evaluation and analytical first
derivatives in Python. The STEP evaluator uses OCP's B-spline `D1`; shape
locations are applied by the BRep adaptors. See the official
[BRepAdaptor_Surface reference](https://occt3d.com/dev/doc/refman/html/class_b_rep_adaptor___surface.html)
and [Geom_BSplineSurface reference](https://occt3d.com/dev/doc/refman/html/class_geom___b_spline_surface.html).
The implementation is locally exercised with OCP 7.9.3.1; the online reference
may describe a newer release.

The report contains pole/knot/normalized-weight differences, sampled point
positions and first derivatives, per-sample errors, and fixed allowed errors.
It covers clamped, nonperiodic support curves/surfaces with positive weights and
C1 continuity at internal knots. Tensor poles use u-major, v-minor order.
The only orientation adjustment is the curve reversal specified in the oracle;
no best-fit matching is performed. Near-identical parameter endpoints may be
affinely mapped within the fixed knot tolerance, with derivative scaling.

This gate does not certify trim curves, pcurves, oriented face normals, areas,
volumes, watertightness, or a global geometric error bound. It does not change
parser provenance/exactness flags. OCP remains a validation-only dependency.

## Rectangular NURBS boundary diagnostics

`scripts/validate_nurbs_trim.py` extends a pinned, passing support oracle with
explicit face/coedge-to-STEP bindings. The
[trim oracle schema](../schemas/nurbs-trim-oracle.schema.json) pins that support
oracle by hash and size and fixes a UV tolerance before comparison.

```sh
python scripts/validate_nurbs_trim.py trim-oracle.json fixture-root --output trim.json
```

This bounded diagnostic accepts one four-edge outer wire per selected NURBS
face, clamped nonperiodic support, and derived linear isoparametric pcurves.
Native curve intervals are derived from vertex positions by line projection or
unique full-support NURBS endpoint matching. Source ranges stay unchanged.
When the public API supplies a derived interval, the diagnostic independently
checks its parameters, method, and endpoint error, and retains it in the report.
Present source ranges, alternate use curves, holes, seams, partial NURBS trims,
and other pcurve parameterizations are rejected until their semantics are
validated. STEP pcurves must be lines or nonrational two-pole degree-1 splines.

The report separates three gates:

- `derived_boundary_gate_passed`: curve/surface-lift positions, UV coordinates,
  first tangents per normalized traversal fraction, and closed rectangular
  boundaries agree at the fixed samples/tolerances.
- `oriented_trim_gate_passed`: directed edges, cyclic wire order, and face sense
  agree. Endpoint alignment for the geometry diagnostic is recorded and does
  not turn reversed traversal into an orientation pass.
- `source_trim_gate_passed`: remains false for this derived-interval profile;
  native source trim metadata is not certified by these measurements.

`comparison_completed=true` distinguishes completed measurements from invalid
inputs or missing dependencies. The CLI returns 1 unless all gates pass, so a
completed diagnostic can deliberately return 1. No parser flags are upgraded.

The reference pcurves come from OCP's imported STEP B-Rep and may be reconstructed
during transfer; they are not a SolidWorks API pcurve capture. Wire exploration
must cover every unique edge. See the official
[BRepTools_WireExplorer reference](https://occt3d.com/dev/doc/refman/html/class_b_rep_tools___wire_explorer.html)
and [BRepAdaptor_Curve2d reference](https://occt3d.com/dev/doc/refman/html/class_b_rep_adaptor___curve2d.html).
Local checks pin OCP 7.9.3.1; the online documentation may describe a newer version.
These finite samples do not establish a global error bound or oriented normals.
