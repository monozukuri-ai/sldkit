# Typed document facts

`parse_file` and `parse_bytes` expose additional source facts that downstream
drafting and bill-of-materials tooling needs. All of them keep the source
provenance rules of the rest of the model: exact tokens stay strings, inferred
interpretations carry `ValueOrigin.INFERRED`, and absent data stays `None`.

## Assembly components

`AssemblyComponent` adds typed projections of `swReference` attributes that
previously stayed in `raw_attributes`:

| Field | Source attribute | Meaning |
|---|---|---|
| `is_virtual` | `swIsVirtualComponent` | The referenced document is saved inside this assembly |
| `is_flexible` | `swFlexible` | Flexible subassembly occurrence |
| `instance_number` | `swReferenceNumber` | Instance number shown after the component name |
| `component_id` | `swID` | Assembly-local identity used by display states |
| `raw_transform` | `swTransform` | The sixteen exact decimal tokens in source order |

`raw_transform` is retained as evidence; sldkit exposes no placement API. On
controlled files the sixteen tokens read as a row-major 4x4 matrix for row
vectors in metres: tokens 0-11 are three rotation rows whose fourth token is
`0`, tokens 12-14 are the translation of the component origin in the root
assembly frame, and token 15 is `1`, so a point `v` of the referenced part is
placed as `v * R + t`. That reading reproduces the vendor's STEP export of one
assembly with 24 root-level occurrences and 14 distinct rotation blocks; the
files, measures, and the rejected transposed reading are recorded under
[observed evidence](development/assembly-validation.md#observed-evidence) in
the assembly validation guide. It is an observation, not a documented
contract. Whether a nested occurrence is relative to its parent or the root,
flexible subassemblies, mirrored or scaled occurrences, and
configuration-specific transforms remain unverified, and the capture workflow
in that guide is still what must pass before a placement API is exposed.

`Configuration.bounding_box` carries the saved `swBoundingBox` tokens of the
configuration root model as `min` and `max` triples. The metre length unit is an
inference cross-checked against decoded Part geometry extents of a controlled
fixture, not a declared source field.

## Material

`Configuration.material` is the localized material label saved in the
`KeyWords` XML, for example a "not specified" placeholder in the authoring
language. It is not resolved against a material library and carries no density.

## Unit settings

`SourceDocument.units` decodes the `SW_Unit*` system properties into
`DocumentUnits`. Each `UnitSetting` keeps the exact integer `code` as a source
value and adds an inferred `name` by applying the `SolidWorks` API enumeration
named in its evidence:

| Field | Property | Enumeration |
|---|---|---|
| `unit_system` | `SW_UnitSystem` | `swUnitSystem_e` |
| `length`, `dual_length`, `mass_property_length` | `SW_UnitsLinear`, `SW_UnitsDualLinear`, `SW_UnitsMassPropLength` | `swLengthUnit_e` |
| `angle` | `SW_UnitsAngular` | `swAngleUnit_e` |
| `mass` | `SW_UnitsMassPropMass` | `swUnitsMassPropMass_e` |
| `volume` | `SW_UnitsMassPropVolume` | `swUnitsMassPropVolume_e` |

Decimal-place counts are exposed as source integers. A code outside the known
members keeps `name=None` with a `units.code_unrecognized` diagnostic, and
conflicting values for one property withhold the setting with
`units.value_conflict`. The property names are not documented by the vendor;
the enumeration assignment follows their naming and is therefore inferred.

## Feature records

`SourceDocument.feature_records` inventories the saved `FeatureManager` items of
a Part or Assembly from the `KeyWords` XML: the XML element name such as
`Extrusion`, `HoleWizard`, `Sketch`, or `Feature`, the localized name and type
labels, dissection links, and `Dimension` children as display text (for example
`<MOD-DIAM>3.57`). This is an index of saved names and display values. It is
not a feature-history reconstruction and does not carry geometry; the binary
feature data remains unsupported.

## Drawing sheets

`DrawingSheet` adds the sheet facts present in the `KeyWords` XML and the
`SW-All Sheet Format Data` system property:

- `paper_size` text and `paper_size_code`, with an inferred `paper_size_name`
  from `swDwgPaperSizes_e`. The user-defined, A3, and A1 codes were
  cross-checked against saved sheet dimensions in controlled Drawings.
- `scale_numerator` and `scale_denominator` as exact decimal tokens.
- `sheet_format_path`, the stored template path, unresolved.
- `custom_property_view`, the view whose model fills the sheet format.
- `width` and `height` from the format-data record of the same sheet name.
  Metres are an inference cross-checked against ISO sheet sizes.
- `format_data`, the ten raw fields of that record in source order for the
  fields whose role is not yet assigned.

A property whose layout differs from the observed ten-field records is kept
whole and reported with `modern.drawing_sheet_format_data_unrecognized`.

## Embedded documents

`SourceDocument.embedded_documents` lists complete documents stored inside the
container, currently the `VirtualComp/<name>` streams that hold virtual
components. Each entry records the container entry ID, stream path, decoded
size, SHA-256, and the probed envelope. Parsing does not descend into them;
extract the entry with `extract_file` and parse or decode the bytes as a
separate input, or let [project scanning](project-scanning.md) resolve virtual
components to them.
