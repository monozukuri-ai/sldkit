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
