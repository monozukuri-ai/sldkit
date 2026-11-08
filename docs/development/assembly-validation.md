# Assembly placement evidence

Occurrence placement remains unsupported by the native parser. It preserves
`swReference` attributes, including `swTransform`, in
`AssemblyComponent.raw_attributes` and as `raw_transform`. A 16-number XML
attribute alone is not enough to establish matrix ordering, length units,
transform direction, or whether a nested occurrence is relative to its parent
or the root assembly. The [observed evidence](#observed-evidence) below settles
the first three for root-level occurrences of one file by comparing against
the vendor's own STEP export; the nested, flexible, mirrored, and
configuration cases are still open, and the capture workflow remains the
acceptance path for a placement API.

## Capture independent facts

Save and close a Pack and Go assembly whose references are inside one project
directory. On Windows with SolidWorks installed and no existing session:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\scripts\capture_assembly_transforms.ps1 `
  -ProjectRoot C:\fixtures\assembly `
  -AssemblyPath Placement.SLDASM `
  -OutputPath C:\fixtures\assembly\Placement.api.json
```

Omit `-Configuration` to observe the configuration selected on reopening, or
pass its exact name to capture an explicitly activated configuration. Capture
each required configuration separately. The script opens silent and read-only,
does not save, and records the source SHA-256 before and after observation.
Selecting/resolving a configuration may cause SolidWorks to evaluate its state
in memory; the capture does not prove native delta replay reproduces that state.

The version-1 JSON contains:

| Field | Contract |
|---|---|
| `source` | Project-relative path, native SHA-256/size, open error/warning codes, configuration selected on reopening |
| `configuration` | Configuration actually observed after optional activation |
| `solidworks_revision` | Exact API revision string |
| `occurrences[].id`, `parent_id` | Capture-local traversal IDs; root children have a null parent |
| `name` | Exact SDK `Name2`, without prepending ancestors again |
| `reference` | Basename, project-relative path when inside the project, SHA-256 when that file exists |
| `referenced_configuration` | Exact SDK configuration name |
| `suppression_code`, `visibility_code` | SDK codes without conflating suppression and visibility |
| `transform.status` | `captured` or `unavailable`; absence is never identity |
| `transform.array_data` | Unmodified 16-value `Transform2.ArrayData` |
| `transform.probes` | Origin and three 1-mm basis points, in meters before and after SDK transformation |
| `children_status` | `enumerated` or `unavailable`; a suppressed component can return no children |

The SDK applies the point transformation itself. This provides independent
evidence for array direction/order without implementing that calculation in
the capture tool. All returned numeric values must be finite. Out-of-project
references have no project path/hash; basenames alone are not resolved matches.
Missing transforms and unenumerated subtrees remain explicit, and capture-local
IDs are not persistent IDs or automatic native-to-API bindings.

## Controlled cases and acceptance

Start with these saved baseline/variant pairs, changing one operation per pair:

1. Two occurrences of the same asymmetric Part, with one translated separately
   along X, Y, and Z in separate variants.
2. Rotate one occurrence about each axis; include one arbitrary-angle variant.
3. Nest that Part inside a translated and rotated subassembly, then place the
   subassembly twice with different root placements.
4. Repeat with configuration changes, hidden/suppressed occurrences, and an
   unresolved reference. Flexible subassemblies and mirrored/scaled occurrences
   need separate cases before those profiles can be supported.

Keep native files, referenced closure, one API JSON per state, revision, exact
operation, hashes, and redistribution status together. Associate source
occurrences uniquely using hierarchy, reference, and configuration evidence.
Compare all four SDK probe points, not only translations; declare tolerances
before comparison. Require native/API hash identity and distinguish evaluated
configuration state from saved source state. Establish parent/root composition
with nested cases before exposing `local_transform` or `world_transform`.
Until those cases pass, `raw_transform` stays source evidence with an observed
layout, not a placement.
Mate reconstruction is a later, separate capability.

## Validation scope and API basis

`pwsh -NoProfile -File scripts/test_assembly_capture.ps1` parses the script and
tests numeric validation and SDK-probe capture with mocks on Linux. This is not
Windows COM execution. Native placement semantics remain unverified until
saved/reopened native files and independent API captures pass the acceptance
criteria above.

The API basis is
[`IComponent2.Transform2`](https://help.solidworks.com/2019/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IComponent2~Transform2.html),
[`IMathPoint.MultiplyTransform`](https://help.solidworks.com/2015/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IMathPoint~MultiplyTransform.html),
and
[`IComponent2.GetChildren`](https://help.solidworks.com/2019/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IComponent2~GetChildren.html).
Those API contracts do not specify the native XML `swTransform` encoding.

## Observed evidence

Recorded 2026-10-23 with sldkit 0.3.0. This is a differential against the
vendor's STEP export of the same assembly, not the API capture above. It
establishes the token layout for root-level occurrences of the observed files
and nothing about the open cases.

### Files

| File | Facts |
|---|---|
| `VALVULA 5-2 VIAS PNEUMATICA SIMPLES SOLENOIDE.SLDASM` | FESTO manifold from the FreeCAD parts library (`Electro-pneumatic/Manifolds/FESTO`, commit `8a6a19e1`); 825,003 bytes, SHA-256 `45608e9c8c4abee5a75e5b292cb22820f0a1d954cb3ab668cfb6465d34e0bc3c`; modern chunk container, `internal_version` 10000; 25 root-level components, 1,421 face tables, 43,094 triangles |
| `VALVULA 5-2 VIAS PNEUMATICA SIMPLES SOLENOIDE.STEP` | Sibling export in the same directory; 6,602,019 bytes, SHA-256 `ab3ec74e8ac10c17a8bb97b2d91c357e455590604bd0b152398d632e91bff583`; 13 products, 24 occurrences |
| `FAN.SLDASM` | Public CPU-fan sample; 3,117,289 bytes, SHA-256 `68108ee16d14e49cc06330455a77e790383b33c31bf61d9472f00be4c5a55399`; `internal_version` 9000; 5 components including two virtual ones, 2,416 face tables |

Neither assembly is redistributed with sldkit; both were used for local
validation only.

### Method

1. `parse_file` for the components and their `raw_transform` tokens, and
   `decode_display_mesh_file` for the tables.
2. Join each face table to its instance through the `uoTempPartTessData_c`
   record that precedes it in the decoded `DisplayLists` stream (see
   [display meshes](../display-mesh.md#coverage)), and the instance path
   `<name>-<number>@<assembly>` to the component with that `instance_name`
   and `instance_number`. This join lives outside sldkit, in
   `cad3d-ir-sldprt` 0.2.2.
3. Read the tokens as a row-major 4x4 for row vectors in metres and place the
   vertices of each instance as `v * R + t`.
4. Read the STEP with `cad3d-ir-step` 0.2.0 (OCCT tessellation, linear
   deflection 0.0005 of the diagonal), bring both results to world-space
   points through their GLB node matrices, and compare bounds and
   nearest-neighbour distances over 40,000 sampled points in each direction
   with a grid cell of 4.18 mm (1 % of the diagonal).

### Findings on the manifold

- All 25 `swReference` entries carry 16 finite decimal tokens. Tokens 3, 7,
  and 11 are `0` and token 15 is `1` in every entry. One entry (`Peça1-1`,
  suppressed and hidden) is the identity; the other 24 use 14 distinct
  rotation blocks, among them 180° about Y (`-1 … 1 … -1`), axis
  permutations, and rotations of about 33°, 30°, 60°, and 3°, with float
  noise such as `2.562996004052438e-019` inside otherwise exact rows.
- Tokens 12-14 scaled by 1000 equal the translations of the 24 STEP
  occurrences `NAUO1` to `NAUO24`, matched by product, to the printed
  0.001 mm. Example: the component
  `VALVULA. 5-2 VIAS PNEU. SIMPLES SOLEN. 1-8 12VDC (V50A513A-A212A)-2` stores
  `1 0 0 0 0 1 0 0 0 0 1 0 -0.1413918515837503 0.01350010307764066
  0.004650000000000001 1`, and `NAUO1` of the same product sits at
  (-141.392, 13.5, 4.65) mm. The suppressed component has no STEP occurrence.
- The 12 instance records in the stream (one per unsuppressed part document)
  all join to a component; no table is left unassigned and no record is
  nested.
- Before placement the stored tables span 46.5 x 89.7 x 210.0 mm, with the
  four valve bodies overlapping at the origin. After placement the bounds
  equal the STEP's to the printed 0.1 mm (356.8 x 168.0 x 138.5 mm,
  diagonal 418.0 mm). Nearest-neighbour distances placed to STEP: median
  0.16 mm, p95 0.81 mm, max 8.2 mm, 0.2 % of samples beyond the cell. STEP
  to placed: median 0.16 mm, p95 3.46 mm, 16.0 % beyond the cell, consistent
  with the 12 occurrences that have no tables in the stream.
- Reading the rotation block transposed (column vectors) keeps the bounds,
  because the valve bodies carry identity or 180° rotations that are their
  own transpose, but raises placed-to-STEP p95 to 2.64 mm and max to 10.7 mm
  and the STEP-to-placed share beyond the cell to 21.1 %. The row-vector
  reading is the one that matches.

### Findings on the fan

All 5 instance records join to components, including the virtual ones
(`Part1wires^Fan-1`, `Part2^Fan-1`), and `NO 1, HOUSING-1` is the only
identity transform. There is no independent export of this file, so its
placement is not verified; it shows only that the join holds on a second file
from a different internal version.

### Not established

Nested occurrences (no sample; a nested record would carry two `@`), flexible
subassemblies, mirrored or scaled occurrences, configuration-specific
transforms, the meaning of hidden or suppressed beyond the one suppressed
identity entry, and the API capture itself. The acceptance criteria above are
unchanged.
