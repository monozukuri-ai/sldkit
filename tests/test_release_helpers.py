from __future__ import annotations

import runpy
from pathlib import Path

import pytest

ROOT = Path(__file__).parents[1]


def test_release_artifact_directory_expansion(tmp_path: Path):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_artifacts.py"))
    wheel = tmp_path / "sldkit-0.1.0-cp313-cp313-any.whl"
    sdist = tmp_path / "sldkit-0.1.0.tar.gz"
    ignored = tmp_path / "checksums.txt"
    wheel.touch()
    sdist.touch()
    ignored.touch()

    assert namespace["_expand_artifacts"]([tmp_path]) == [wheel, sdist]


def test_release_artifact_directory_must_not_be_empty(tmp_path: Path):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_artifacts.py"))

    with pytest.raises(AssertionError, match="artifact directory is empty"):
        namespace["_expand_artifacts"]([tmp_path])


def test_release_artifact_rejects_private_design_material(tmp_path: Path):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_artifacts.py"))

    with pytest.raises(AssertionError, match="forbidden directory"):
        namespace["_check_names"](
            tmp_path / "sldkit-0.1.0.tar.gz",
            ["sldkit-0.1.0/designs/private-note.md"],
        )


def test_release_artifact_rejects_obsolete_vendored_shared_reader(tmp_path: Path):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_artifacts.py"))

    with pytest.raises(AssertionError, match="obsolete vendored shared reader"):
        namespace["_check_names"](
            tmp_path / "sldkit-0.2.0.tar.gz",
            ["sldkit-0.2.0/vendor/parasolid-core/src/lib.rs"],
        )


@pytest.mark.parametrize(
    "relative",
    [
        "docs/parser-progress-2026-09-26.ja.md",
        "docs/part-boundary-followup.ja.md",
        "docs/development/local-session-notes.md",
        "scripts/create_partial_nurbs_fixture.swb",
        "scripts/create_partial_nurbs_step.py",
        "scripts/validate_m5_solidworks_fixture.py",
    ],
)
def test_release_artifact_rejects_unlisted_docs_and_scripts(tmp_path: Path, relative):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_artifacts.py"))

    with pytest.raises(AssertionError, match="unlisted documentation or script"):
        namespace["_check_names"](
            tmp_path / "sldkit-0.2.0.tar.gz", [f"sldkit-0.2.0/{relative}"]
        )


def test_release_artifact_accepts_public_developer_docs_and_tools(tmp_path: Path):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_artifacts.py"))
    namespace["_check_names"](
        tmp_path / "sldkit-0.2.0.tar.gz",
        [
            "sldkit-0.2.0/docs/geometry.md",
            "sldkit-0.2.0/docs/development/releasing.md",
            "sldkit-0.2.0/scripts/check_license.py",
            "sldkit-0.2.0/scripts/test_assembly_capture.ps1",
        ],
    )


def test_release_artifact_rejects_non_abi3_wheel(tmp_path: Path):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_artifacts.py"))
    wheel = tmp_path / "sldkit-0.1.0-cp313-cp313-any.whl"
    wheel.touch()

    with pytest.raises(AssertionError, match="not Python 3.10\\+ ABI3"):
        namespace["_check_wheel"](wheel)


def test_wheel_smoke_requires_exactly_one_wheel(tmp_path: Path):
    namespace = runpy.run_path(str(ROOT / "scripts/smoke_wheel_artifact.py"))
    resolve_wheel = namespace["resolve_wheel"]

    with pytest.raises(ValueError, match="found 0"):
        resolve_wheel(tmp_path)
    first = tmp_path / "first.whl"
    first.touch()
    assert resolve_wheel(tmp_path) == first.resolve()
    (tmp_path / "second.whl").touch()
    with pytest.raises(ValueError, match="found 2"):
        resolve_wheel(tmp_path)


def test_release_versions_and_tag_match():
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_version.py"))
    version = namespace["source_version"](ROOT)

    assert namespace["verify_release_version"](ROOT, f"v{version}") == version


def test_release_tag_must_match_source_version():
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_version.py"))

    with pytest.raises(ValueError, match="does not match"):
        namespace["verify_release_version"](
            ROOT, f"v{namespace['source_version'](ROOT)}-mismatch"
        )


@pytest.fixture
def version_checkout(tmp_path):
    import shutil

    paths = [
        "Cargo.toml",
        "Cargo.lock",
        "pyproject.toml",
        "uv.lock",
        "fuzz/Cargo.toml",
        "fuzz/Cargo.lock",
        "viewer/package.json",
        "viewer/package-lock.json",
    ]
    paths.extend(str(p.relative_to(ROOT)) for p in ROOT.glob("crates/*/Cargo.toml"))
    for relative in paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / relative, destination)
    return tmp_path


@pytest.mark.parametrize(
    ("cargo", "python"),
    [("1.2.3", "1.2.3"), ("1.2.3-rc.1", "1.2.3rc1"), ("1.2.3-dev0", "1.2.3.dev0")],
)
def test_python_release_version_spelling(cargo, python):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_version.py"))
    assert namespace["python_version"](cargo) == python


@pytest.mark.parametrize(
    "version", ["v1.2.3", "1.2", "01.2.3", "1.2.3-rc.01", "1.2.3+local"]
)
def test_invalid_bump_leaves_files_untouched(version_checkout, version, monkeypatch):
    namespace = runpy.run_path(str(ROOT / "scripts/bump_version.py"))
    before = {p: p.read_bytes() for p in version_checkout.rglob("*") if p.is_file()}

    def unexpected_command(*args, **kwargs):
        pytest.fail("invalid version must be rejected before running tools")

    monkeypatch.setattr(namespace["subprocess"], "run", unexpected_command)
    with pytest.raises(ValueError, match="unsupported release version"):
        namespace["bump_version"](version, version_checkout)
    assert {p: p.read_bytes() for p in before} == before


@pytest.mark.parametrize("filename", ["Cargo.lock", "fuzz/Cargo.lock"])
@pytest.mark.parametrize("damage", ["stale", "missing"])
def test_release_rejects_stale_or_missing_local_lock(
    version_checkout, filename, damage
):
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_version.py"))
    path = version_checkout / filename
    blocks = path.read_text().split("[[package]]")
    for index, block in enumerate(blocks):
        if '\nname = "sldkit-core"\n' in block:
            current = namespace["source_version"](version_checkout)
            blocks[index] = (
                block.replace(f'version = "{current}"', f'version = "{current}-stale"')
                if damage == "stale"
                else block.replace('name = "sldkit-core"', 'name = "missing"')
            )
            break
    else:
        pytest.fail("fixture must contain the workspace crate")
    path.write_text("[[package]]".join(blocks))
    with pytest.raises(ValueError, match=f"{filename}: stale or missing sldkit-core"):
        namespace["verify_release_version"](version_checkout)


@pytest.mark.parametrize("damage", ["python", "crate", "uv", "viewer", "npm_lock"])
def test_release_rejects_duplicate_version_sources(version_checkout, damage):
    import json

    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_version.py"))
    version = namespace["source_version"](version_checkout)
    if damage == "python":
        path = version_checkout / "pyproject.toml"
        path.write_text(
            path.read_text().replace('dynamic = ["version"]', f'version = "{version}"')
        )
    elif damage == "crate":
        path = version_checkout / "crates/sldkit-core/Cargo.toml"
        path.write_text(
            path.read_text().replace(
                "version.workspace = true", f'version = "{version}"'
            )
        )
    elif damage == "uv":
        path = version_checkout / "uv.lock"
        path.write_text(
            path.read_text().replace(
                'name = "sldkit"\n', f'name = "sldkit"\nversion = "{version}"\n'
            )
        )
    else:
        path = version_checkout / (
            "viewer/package.json" if damage == "viewer" else "viewer/package-lock.json"
        )
        value = json.loads(path.read_text())
        value["version"] = version
        path.write_text(json.dumps(value))
    with pytest.raises(ValueError):
        namespace["verify_release_version"](version_checkout)


@pytest.mark.parametrize("failed_step", [1, 2, 3, 4])
def test_failed_bump_restores_existing_edits(
    version_checkout, monkeypatch, failed_step
):
    import subprocess

    namespace = runpy.run_path(str(ROOT / "scripts/bump_version.py"))
    manifest = version_checkout / "Cargo.toml"
    manifest.write_text(manifest.read_text() + "\n# unrelated local edit\n")
    before = {
        name: (version_checkout / name).read_bytes()
        for name in namespace["MANAGED_FILES"]
    }
    calls = []

    def run(command, *, cwd, check):
        assert cwd == version_checkout and check
        assert "--offline" in command
        calls.append(command)
        # Simulate partially written lockfiles, including a failure after all tools.
        for name in namespace["MANAGED_FILES"][1:]:
            (cwd / name).write_text("incomplete update\n")
        if len(calls) == failed_step:
            raise subprocess.CalledProcessError(1, command)

    monkeypatch.setattr(namespace["subprocess"], "run", run)
    with pytest.raises((subprocess.CalledProcessError, ValueError)):
        namespace["bump_version"]("9.8.7", version_checkout, offline=True)
    assert len(calls) == min(failed_step, 3)
    assert {name: (version_checkout / name).read_bytes() for name in before} == before
