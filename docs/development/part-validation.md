# Part validation

These tools compare a bounded native Part profile with independently captured
SolidWorks API values. They do not change the parser's provenance or support
status. See [Part geometry](../geometry.md) for the public model and limits.
Run the commands from the repository or extracted source-distribution root.

## Orientation and analytic boundaries

Save and reopen a Part in SolidWorks, then run
[`capture_part_orientation.swb`](../../scripts/capture_part_orientation.swb)
with that document active and no unsaved edits. The macro observes the active
document without rebuilding or saving it. It writes `part-orientation.csv`
beside the source and refuses to overwrite an existing capture. Coordinates
are in meters; senses and parameter ranges retain the API values.

Record the source and capture SHA-256 values before validation. Replace
`SOURCE_SHA256` and `CAPTURE_SHA256` below with those values:

```sh
python scripts/validate_part_orientation.py part.SLDPRT part-orientation.csv \
  --source-sha256 SOURCE_SHA256 --capture-sha256 CAPTURE_SHA256 \
  --output orientation.json
```

The orientation profile accepts one solid with plane/cylinder faces and
line/circle/ellipse edges. It checks native coedge traversal and surface
orientation at API midpoints, excluding explicitly derived seams.

`validate_part_boundaries.py` additionally checks directed intervals, loop
order, analytic outer/inner roles, and curve-to-pcurve-to-surface consistency.
Cylinder area qualification requires a STEP export of the same saved Part and
an OCP installation:

```sh
python scripts/validate_part_boundaries.py part.SLDPRT part-orientation.csv \
  --source-sha256 SOURCE_SHA256 --capture-sha256 CAPTURE_SHA256 \
  --step part.step --step-sha256 STEP_SHA256 --output boundaries.json
```

Cylinder faces may be split differently in STEP. The comparison groups them by
axis line and radius before integrating area. API area is retained as a
diagnostic. Omitting STEP when cylinders are present leaves the overall result
unqualified. Stored trim and general periodic loop roles remain separate,
unverified capabilities even when these bounded checks pass.

## NURBS sample comparison

[`validate_partial_nurbs.py`](../../scripts/validate_partial_nurbs.py) accepts a
version-2 sample CSV from a saved/reopened Part with one sheet body, one face,
and four open, nonperiodic B-spline edges. It requires `CAPTURE`, `SOURCE`,
`UNITS`, `BODY`, `FACE`, `EDGE`, `SUPPORT`, `SAMPLE`, and final `COMPLETE` records.
Each edge must have 17 evenly spaced parameter samples with positions and first
derivatives in source units (meters), plus its API interval and support domain.
The version-1 orientation capture above does not contain these sample records.

The command takes `source`, `capture`, `--source-sha256`, `--capture-sha256`,
and `--output`. Add `--require-partial` to require a proper subset of at least
one native support domain. Endpoint matching must be unique; sampled positions
and directed tangents must agree after the recorded affine parameter mapping.
Reparameterizing every edge to its full support cannot qualify partial-interval
derivation. Missing decoded geometry, incomplete captures, and unsupported
profiles fail the check. Native partial-interval qualification remains pending.
