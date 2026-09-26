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
    versions = namespace["release_versions"](ROOT)
    version = namespace["verify_release_version"](
        ROOT, f"v{next(iter(versions.values()))}"
    )

    assert set(versions.values()) == {version}


def test_release_tag_must_match_source_version():
    namespace = runpy.run_path(str(ROOT / "scripts/verify_release_version.py"))

    with pytest.raises(ValueError, match="does not match"):
        namespace["verify_release_version"](ROOT, "v999.0.0")
