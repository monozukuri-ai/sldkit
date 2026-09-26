# Release checks

Version 0.2.0 introduces the new-material licensing boundary described in
[licensing](license.md) and pins the shared registry reader to `parasolid-core 0.2.0`.
Keep previously published MIT and third-party permissions intact. Use a new
version for corrections; do not replace existing public artifacts with different terms.

## Source and notice updates

Use Python 3.11+ for the release-check scripts. After changing dependencies,
update the main, vendor and fuzz lockfiles with Cargo, then fetch all locked
sources. The notice refresh is offline and reads the local Cargo registry.

```bash
cargo fetch --locked
cargo fetch --locked --manifest-path vendor/cadmpeg-codec-sldprt/Cargo.toml
cargo fetch --locked --manifest-path fuzz/Cargo.toml
python scripts/sync_license_notices.py --refresh-rust
npm ci --prefix viewer
npm run build --prefix viewer
python scripts/check_license.py
python scripts/verify_release_version.py --tag v0.2.0
```

For a legal-text change without dependency changes, omit `--refresh-rust`.
The generator keeps the canonical PolyForm and legacy MIT texts unchanged,
records selected dependency terms and regenerates crate and viewer notices.
New license combinations and exclusions require review in the generator/checker.
The catalog includes build and target-specific tools; it is not a runtime SBOM.

Run the source tests, patched backend tests and Viewer tests. The archive
checks include negative tests for missing/stale notices, wrong metadata paths,
incorrect dependency checksums and incomplete license inventories.

## Distribution qualification

Build into a fresh directory. Old local wheels may have the same filename
version but different code or Python tags; do not combine them with a new build.

```bash
maturin build --release --locked --out dist/release
maturin sdist --out dist/release
python scripts/verify_release_artifacts.py dist/release
python scripts/smoke_wheel_artifact.py dist/release
```

The wheel check requires Python 3.10+ ABI3, exact `License-Expression` and
`License-File` declarations, canonical notice bytes and current viewer assets.
The installed smoke also generates standalone HTML and checks its complete
notice bundle outside the checkout. The sdist check checks the registry reader's
version/checksum, Apache vendor boundary, source notices and required files.
Private CAD files, corpus, references, internal notes and local preview output
must remain outside both archives.

Rebuild a wheel from the extracted sdist as the workflows do. Maturin reduces
the Cargo workspace; Cargo metadata may first prune workspace-only packages
from its copied lockfile. Verify that the new wheel retains the release's exact
license files and metadata, then run the installed smoke again.

CI qualifies Linux x86_64, Windows x86_64 and macOS arm64 wheels, tests Python
3.10 and 3.14 installation, and performs a Linux sdist round-trip. Local tests
do not substitute for those remote jobs. Source checks must run from a checkout
containing the three development workspaces; an extracted sdist intentionally
omits fuzz and workspace-only CLI sources.

`workflow_dispatch` builds without publication. A new `v*` tag publishes the
verified bundle through GitHub Release and then PyPI. Check the downloaded
files' SHA-256 and licensing metadata against the qualified bundle. Do not
overwrite an existing release as part of a licensing change.

## PyPI Trusted Publisher setup

Before the first tag release, register a **pending publisher** on the PyPI
account's [Publishing page](https://pypi.org/manage/account/publishing/).
Use these GitHub Actions settings:

| PyPI field | Value |
| --- | --- |
| PyPI project name | `sldkit` |
| Owner | `monozukuri-ai` |
| Repository name | `sldkit` |
| Workflow name | `release.yml` |
| Environment name | `pypi` |

The workflow field is the filename, without `.github/workflows/`, rather than
the workflow's display name (`Release`). Use the production PyPI site; TestPyPI
publisher registrations are separate. A pending publisher creates the project
on its first successful publication. For an existing project, manage publishers
on the project's [Publishing page](https://pypi.org/manage/project/sldkit/settings/publishing/).
Registration requires a maintainer's PyPI login. The workflow already grants
`id-token: write` to the publishing job and uses the `pypi` environment.

See PyPI's instructions for [new projects](https://docs.pypi.org/trusted-publishers/creating-a-project-through-oidc/)
and [existing projects](https://docs.pypi.org/trusted-publishers/adding-a-publisher/).

## Recovering a PyPI authentication failure

`invalid-publisher` means that PyPI received a valid OIDC token but could not
match it to a registered publisher. Compare the failed job's repository,
workflow filename and environment claims with the settings above, including
after a repository move or rename. Correct the registration on PyPI; changing
the package version or license metadata does not fix this authentication error.
See [PyPI's troubleshooting guide](https://docs.pypi.org/trusted-publishers/troubleshooting/).

If only `Publish to PyPI` failed at token exchange and PyPI has no files for the
version, fix the publisher and rerun the failed job using the existing verified
bundle. Replace `RUN_ID` with the failed Release workflow run's numeric ID:

```bash
gh run rerun RUN_ID --repo monozukuri-ai/sldkit --failed
```

The GitHub Release may already be public. Keep its tag and artifacts intact;
rerunning all jobs would rebuild and upload them again. `workflow_dispatch`
does not publish. If an upload partially succeeded, compare the existing PyPI
files and hashes with the verified bundle before retrying; the publish action
does not skip existing files. After recovery, confirm that PyPI has all three
wheels and the sdist with the same SHA-256 hashes as the GitHub Release.
