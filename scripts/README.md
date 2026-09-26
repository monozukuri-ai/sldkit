# Development scripts

These are contributor and maintainer tools. The installed package exposes the
`sldkit` command; these scripts run from a repository checkout or, where listed,
an extracted source distribution. See the
[developer guides](../docs/development/README.md) for capture and release workflows.

| Tools | Purpose | Environment |
|---|---|---|
| `check_license.py`, `sync_license_notices.py`, `verify_release_version.py`, `verify_release_artifacts.py` | License notices, version consistency, and distribution contents | Python 3.11+; notice refresh also needs the locked Cargo sources |
| `smoke_wheel_artifact.py`, `smoke_installed_package.py` | Install one wheel into an isolated venv and exercise the installed package | Python with `venv` and `ensurepip`; see the release guide |
| `capture_part_orientation.swb` | Observe a saved active Part's body/face/coedge facts | Interactive SolidWorks |
| `capture_assembly_transforms.ps1`, `capture_drawing_ground_truth.ps1` | Capture independent occurrence or sheet/view facts | Windows PowerShell 5.1 and SolidWorks |
| `test_assembly_capture.ps1` | Syntax and mock checks for the Assembly capture | PowerShell; no SolidWorks required |
| `compare_drawing_structures.py` | Compare two Drawing inventories and optional API captures | Python development environment |
| `capture_step_geometry.py`, `validate_geometry_oracle.py` | Capture STEP facts and compare a pinned geometry oracle | Python development environment; OCP for STEP capture |
| `validate_part_orientation.py`, `validate_part_boundaries.py` | Bounded Part orientation and analytic boundary checks | Python development environment; OCP for cylinder STEP areas |
| `validate_nurbs_geometry.py`, `validate_nurbs_trim.py`, `nurbs_geometry.py` | NURBS support and rectangular boundary comparisons; shared evaluator | Python development environment and OCP |
| `validate_partial_nurbs.py` | Bounded sheet-edge comparison with version-2 API samples | Python development environment |

All tools in the table and this catalog are included in the source distribution.
They are excluded from wheels. None of the CAD inputs or completed validation
records are distributed.

The following tools are for repository checkouts and are not included in the
source distribution: `validate_container_corpus.py`, `validate_semantic_corpus.py`,
`validate_geometry_corpus.py`, `validate_project_graph.py`, and
`verify_corpus_source.py`. They exercise corpus contracts, Rust/Python parity,
or checkout provenance and may require the Rust CLI or corpus manifest.

When adding a reusable tool, document its inputs and dependencies here, choose
its distribution scope explicitly, and update the artifact verifier accordingly.
