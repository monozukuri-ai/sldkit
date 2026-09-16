# Contributing

Read the [development instructions](README.md#development), [parser provenance](docs/parser-provenance.md)
and [supported scope](docs/compatibility.md).
Keep changes bounded, preserve source values and include evidence for newly
supported file profiles. Update both language versions of the licensing documentation together. Do not submit confidential CAD files or samples without redistribution
permission.

## Contribution licensing

sldkit offers new material under PolyForm Noncommercial 1.0.0 and separate
commercial agreements with UnRobotics Inc. To allow both forms of distribution,
new external code and documentation contributions require explicit acceptance
of [CLA version 1.0](CLA.md). Contributors retain copyright; the agreement grants
nonexclusive rights including commercial sublicensing and specified patents.
This does not retroactively change earlier MIT contributions.

On the pull request, identify the contribution and post the following acceptance
statement, filling in the identity and scope. Reference the immutable Git
revision of CLA.md that you reviewed, not just a moving branch URL:

> I accept the sldkit Contributor License Agreement version 1.0 at
> [full commit SHA / immutable CLA URL] for my contributions in [PR and commit
> range]. I am [individual or legal entity], and I have authority to grant the
> rights stated in that agreement. Third-party material: [none, or list and terms].

If your employer or another entity owns the contribution, obtain authorized
acceptance on behalf of that entity. Contact
[UnRobotics Inc.](https://www.un-robotics.com/#contact) for a private signature
record rather than posting personal addresses or confidential documents.

Maintainers must verify acceptance before merging an external contribution.
Record the contributor/entity, authority, covered commits, CLA revision,
acceptance date and link or private record reference. If a PR gains contributions
from another rights holder, obtain their acceptance as well. This is a manual
review requirement; no CLA bot is currently configured. A DCO sign-off alone
does not record acceptance of this agreement.

Issue reports and suggestions do not require the CLA unless they include
material proposed for incorporation that requires a license grant.

## License and packaging changes

Preserve earlier MIT and third-party notices. After changing legal files, run
`python scripts/sync_license_notices.py`, then rebuild the viewer with
`npm run build --prefix viewer`. Check `python scripts/check_license.py` and
the [distribution gates](docs/releasing.md). Generated Rust and viewer notices
must match the canonical files; do not edit those copies by hand.
