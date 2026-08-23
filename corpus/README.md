# Validation corpus metadata

The repository records validation-input provenance separately from parser code.
SolidWorks CAD binaries and paired neutral exports are not vendored or included
in package artifacts.

- `manifest.jsonl` records immutable artifact hashes, expected facts, rights
  status, and validation methods.
- `sources.lock.json` pins external repository revisions and source paths.
- `licenses/` contains artifact-specific license notes when redistribution has
  been reviewed.
- `synthetic/` is reserved for project-authored negative and boundary inputs.
- `external/` and `cache/` are local-only and ignored by Git.

Manually supplied or not-yet-audited files belong under `external/local/` and
must remain untracked. A repository-level license is not assumed to cover
supplier or third-party CAD files within that repository.

Adding an artifact requires an immutable SHA-256, byte size, provenance state,
redistribution decision, and independently reviewable expected facts. Rights
review and reference-closure validation are separate decisions: an artifact may
be legally usable while its saved project references remain incomplete.

The public metadata and source locks can be checked without downloading CAD
files:

```bash
uv run --frozen pytest tests/test_corpus_contract.py
```

For a source that records `source_path`, clone and check out the locked revision,
then verify that Git LFS materialization, byte sizes, and hashes match:

```bash
python scripts/verify_corpus_source.py \
  --source-id paulggin-cryostat-sample-stage /path/to/checkout
```

The verifier is read-only and performs no network fetch. A hash mismatch is a
failure, including when a checkout contains a Git LFS pointer instead of the CAD
binary. See [parser provenance](../docs/parser-provenance.md) for contribution
requirements.
