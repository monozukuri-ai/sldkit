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
