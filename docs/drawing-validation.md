# Controlled Drawing validation

M6a uses two independent evidence sources for every single-variable Drawing
change:

1. `sldkit` inventories exact XML record ranges and hashes whole candidate
   binary streams.
2. SolidWorks records the authored sheet and view state through its public COM
   API.

The two observations are reported together, but they are not automatically
declared to be the same native record. That correspondence requires repeated
controlled cases and, for binary carriers, verified record framing.

## Authoring a pair

Start from a Pack and Go project whose Drawing references resolve within one
directory tree. Save a baseline, copy it to a variant, perform exactly one
operation in the variant, save, and close SolidWorks. Keep the referenced Part
and Assembly files unchanged. Record the SolidWorks product, service pack, the
operation, and redistribution status in the fixture manifest.

Useful first pairs are:

- one sheet versus a second sheet;
- a base view versus one added projected view;
- the same view before and after one position, scale, or angle change;
- a resolved view versus the same broken reference;
- one added note, dimension, table, or Drawing sketch entity per pair.

Separate files are required. Re-saving one pathname in place loses the exact
baseline bytes needed by the differential.

## Capturing SolidWorks API facts

On Windows with SolidWorks installed, close every existing SolidWorks session
and run Windows PowerShell 5.1:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass `
  -File .\scripts\capture_drawing_ground_truth.ps1 `
  -ProjectRoot C:\fixtures\m6a `
  -DrawingPath M6a-00-Base.SLDDRW `
  -FixtureId m6a-base-view `
  -OutputPath C:\fixtures\m6a\M6a-00-Base.api.json
```

Repeat for the variant. The script refuses an already-running SolidWorks
process, opens the Drawing silent and read-only, and refuses to overwrite an
output unless `-Force` is explicit. Its record is path-free: source and resolved
reference paths are project-relative, and unresolved stored paths are reduced
to basenames.

The capture contains:

- sheet persistent reference and the eight values returned by
  `ISheet.GetProperties2` as named fields;
- view persistent reference, `swDrawingViewTypes_e` code, base-view state,
  referenced model/configuration, and reference-resolution state;
- sheet size and view position in meters, angle in radians, decimal and ratio
  scale, scale inheritance flags, and the 13 values returned by
  `IView.GetViewXform`;
- SolidWorks build/hot-fix and current/saved license type for provenance.

`unavailable`, `none`, `no_reference`, `missing`, and `ambiguous` remain distinct.
The JSON contract is
[`drawing-ground-truth.schema.json`](schemas/drawing-ground-truth.schema.json).

Persistent-reference byte representations may change across rebuilds or
SolidWorks releases. Keep their exact bytes and hashes as API evidence, but do
not treat equal bytes across different authoring environments as an sldkit
stable ID.

## Comparing native evidence

After copying both native files and both API captures to a machine with the
development environment installed, run:

```bash
uv run python scripts/compare_drawing_structures.py \
  fixtures/M6a-00-Base.SLDDRW \
  fixtures/M6a-01-Projected.SLDDRW \
  --baseline-api fixtures/M6a-00-Base.api.json \
  --variant-api fixtures/M6a-01-Projected.api.json \
  --profile service \
  --output fixtures/M6a-00--01.diff.json
```

The comparator first requires each API capture SHA-256 to match its native
Drawing. It then reports four separate deltas:

- exact XML record content and parent hierarchy, grouped by source fields as a
  correspondence heuristic;
- source-supported sheet and direct-view projections;
- whole decoded `Definition`, `DisplayLists`, and `VBLists` candidates;
- independently observed SolidWorks sheet and view facts.

An operation appearing in one API view and one changed candidate stream is a
lead for investigation, not record-framing proof. The report therefore fixes
`binary_record_framing_verified`, `api_to_native_record_mapping_verified`, and
`renderable_semantics_verified` to `false`.

## Handoff gate

A controlled pair is ready for review only when all of these are present:

- baseline and variant `.SLDDRW` files;
- the unchanged referenced Pack and Go closure;
- one API JSON capture per Drawing;
- the machine-generated differential JSON;
- an authoring manifest with the one operation, SolidWorks revision/service
  pack, native hashes, and redistribution status.

Linux-only synthetic tests validate the schema and differential logic. They do
not prove that the PowerShell capture ran, that SolidWorks returned the expected
facts, or that a native binary field has been decoded.

## SolidWorks API basis

The capture contract follows the official API definitions for
[`ISheet.GetProperties2`](https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.ISheet~GetProperties2.html),
[`IView.GetBaseView`](https://help.solidworks.com/2026/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IView~GetBaseView.html),
[`IView.GetViewXform`](https://help.solidworks.com/2026/english/api/sldworksapi/SolidWorks.Interop.sldworks~SolidWorks.Interop.sldworks.IView~GetViewXform.html),
and
[`IModelDocExtension.GetPersistReference3`](https://help.solidworks.com/2023/english/api/sldworksapi/SOLIDWORKS.Interop.sldworks~SOLIDWORKS.Interop.sldworks.IModelDocExtension~GetPersistReference3.html).
