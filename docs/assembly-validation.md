# Assembly placement evidence

Occurrence placement remains unsupported by the native parser. It preserves
`swReference` attributes, including `swTransform`, in
`AssemblyComponent.raw_attributes`. A 16-number XML attribute is not enough to
establish matrix ordering, length units, transform direction, or whether a
nested occurrence is relative to its parent or the root assembly.

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
Until those cases pass, `raw_attributes["swTransform"]` is only source evidence.
Mate reconstruction is a later, separate capability.

## Validation scope and API basis

`pwsh -NoProfile -File scripts/test_assembly_capture.ps1` parses the script and
tests numeric validation and SDK-probe capture with mocks on Linux. This is not
Windows COM execution. No real Assembly API capture was available for the
2026-09-26 implementation; native placement semantics remain unverified.

The API basis is
[`IComponent2.Transform2`](https://help.solidworks.com/2019/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IComponent2~Transform2.html),
[`IMathPoint.MultiplyTransform`](https://help.solidworks.com/2015/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IMathPoint~MultiplyTransform.html),
and
[`IComponent2.GetChildren`](https://help.solidworks.com/2019/English/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IComponent2~GetChildren.html).
Those API contracts do not specify the native XML `swTransform` encoding.
