from __future__ import annotations

from pathlib import Path

import tomllib

ROOT = Path(__file__).parents[1]
FORBIDDEN = {"cad3d-ir", "cadquery", "occt", "pythonocc-core"}


def test_python_runtime_has_no_downstream_ir_or_geometry_kernel_dependency():
    project = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    dependencies = {
        requirement.split("[", 1)[0].split("=", 1)[0].lower()
        for requirement in project["project"]["dependencies"]
    }
    assert dependencies.isdisjoint(FORBIDDEN)


def test_rust_workspace_dependency_boundary():
    for manifest in [ROOT / "Cargo.toml", *(ROOT / "crates").glob("*/Cargo.toml")]:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        dependencies = {
            name.lower()
            for section in ("dependencies", "dev-dependencies", "build-dependencies")
            for name in data.get(section, {})
        }
        assert dependencies.isdisjoint(FORBIDDEN), manifest


def test_private_development_inputs_are_explicitly_excluded_from_artifacts():
    project = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    excluded = set(project["tool"]["maturin"]["exclude"])
    assert {"reference/**", "corpus/**", "designs/**"} <= excluded


def test_public_documentation_is_included_only_in_the_source_distribution():
    project = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    includes = project["tool"]["maturin"]["include"]
    assert {"path": "docs/**/*", "format": "sdist"} in includes


def test_license_files_include_project_and_compiled_dependency_licenses():
    project = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    assert project["project"]["license-files"] == ["LICENSE", "LICENSES/*"]
