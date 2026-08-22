# Parser provenance policy

Every binary-format rule must have reviewable evidence. A plausible field name
or successful parse of one file is not sufficient evidence by itself.

This policy is an engineering control, not legal advice.

## Accepted evidence

- Files authored by the project or supplied with explicit permission, together
  with the creating application version and independently recorded expectations
- Controlled byte differences where one source operation changed at a time
- Public standards for generic containers such as CFB and ZIP
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

## Distribution boundary

External CAD binaries, private fixtures, fetch caches, and validation outputs
are development inputs. They are not included in wheels or source
distributions. A test artifact may be distributed only after its rights and
license obligations have been reviewed explicitly.
