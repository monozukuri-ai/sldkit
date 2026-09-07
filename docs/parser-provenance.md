# Parser provenance policy

Every binary-format rule must have reviewable evidence. A plausible field name
or successful parse of one file is not sufficient evidence by itself.

This policy is an engineering control, not legal advice.

## Accepted evidence

- Files authored by the project or supplied with explicit permission, together
  with the creating application version and independently recorded expectations
- Controlled byte differences where one source operation changed at a time
- Public standards for generic containers such as CFB and ZIP
- Public property-set specifications for standardized OLE summary and custom
  property carriers
- Observable behavior and independently generated exports with immutable input
  hashes, tool versions, and recorded results
- Audited third-party source under a compatible license when provenance and
  attribution obligations are known

Confidential specifications, leaked material, access-control circumvention,
decompiler output from proprietary binaries, and files without authorized use
or redistribution must not be contributed.

## Required evidence for parser changes

A parsing rule beyond a generic envelope signature should record:

- the evidence category and immutable input hashes;
- the controlled observation or public specification supporting the rule;
- the relevant document kind and observed internal versions;
- negative, malformed, and truncation cases;
- diagnostic and coverage behavior outside the supported evidence; and
- any third-party attribution required by intentionally derived code.

Unverified fields remain unknown or preserved. They must not silently become an
empty value, `false`, or a semantic success.

## Public specification baseline

OLE property-set decoding uses Microsoft's public specifications for the
[PropertySetStream structure](https://learn.microsoft.com/en-us/openspecs/windows_protocols/MS-OLEPS/e5484a83-3cc1-43a6-afcf-6558059fe36e),
[typed property values](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-oleps/f122b9d7-e5cf-4484-8466-83f6fd94b3cc),
and [standard summary property identifiers](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-oshared/87667163-ea1e-4d67-9eec-47cad74e8030).
Application-defined records still require independent evidence and remain
preserved when their meaning is not established.

Modern Part geometry decoding uses the pinned Apache-2.0
`cadmpeg-codec-sldprt` 0.5.3 source with the `0.5.3+sldkit.4` patch.
Patch 3 corrects FIN link/end-vertex interpretation only after the verified
native hierarchy/schema gate. Source read spans remain attached to the original
bytes; orientation conversion does not certify numeric source trim intervals.
The [patch record](../vendor/cadmpeg-codec-sldprt/PATCHES.md) identifies the
upstream archive, changed files, evidence, and exact supported schema.
Its published development policy
limits format work to lawfully obtained CAD bytes, public information, and
project-authored experiments, and excludes proprietary SDK-derived or
confidential implementation material. `sldkit` applies its own bounded input,
source model, diagnostics, exact-stream identity, and validation contract at
the public boundary rather than exposing the dependency's interchange model.
The dependency version and source are reviewable in `Cargo.lock` and the
[upstream legal policy](https://github.com/cadmpeg/cadmpeg/blob/v0.5.3/LEGAL.md).

Standard embedded Parasolid headers and the shared partial geometry readers now
come from `parasolid-core 0.1.0-dev6`, with the registry checksum in `Cargo.lock`.
The original core is MIT licensed; adopted cadmpeg/sldkit readers are Apache-2.0
licensed, and the core declares `MIT AND Apache-2.0`. Both license texts remain
in the distribution. Patch 4 removes the adapter's duplicate readers while
retaining unit conversion, public models and source classification.

Core schema framing determines the existing body-domain origin; malformed
numeric framing and limits are covered by adapter tests. The local header reader
is restricted to the two source-less writer keys. Partial known-record reads
and bounded point/topology merging do not prove complete delta application or
final saved state; see the [dependency boundary](geometry.md#parasolid-dependency).

## Distribution boundary

External CAD binaries, private fixtures, fetch caches, and validation outputs
are development inputs. They are not included in wheels or source
distributions. A test artifact may be distributed only after its rights and
license obligations have been reviewed explicitly.
