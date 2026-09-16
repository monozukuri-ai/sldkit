from __future__ import annotations

import json
import runpy
import shutil
from email.message import Message
from pathlib import Path
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).parents[1]
license_check = SimpleNamespace(
    **runpy.run_path(str(ROOT / "scripts/check_license.py"))
)


def archive_license_fixture():
    meta = Message()
    meta["Name"] = "sldkit"
    meta["Version"] = license_check.read_toml(ROOT / "pyproject.toml")["project"][
        "version"
    ]
    meta["License-Expression"] = license_check.DISTRIBUTION_LICENSE
    prefix = "sldkit.dist-info/licenses/"
    contents = {}
    for path in license_check.license_paths():
        meta["License-File"] = path
        contents[prefix + path] = (ROOT / path).read_bytes()
    return meta, prefix, contents


def test_source_notices_and_dependency_catalog_are_current():
    assert license_check.check() == "0.2.0"


def test_archive_accepts_complete_exact_notices():
    meta, prefix, contents = archive_license_fixture()
    license_check.check_archive_licenses(contents.__getitem__, meta, prefix)


@pytest.mark.parametrize(
    "damage",
    [
        "missing",
        "stale",
        "wrong_directory",
        "duplicate",
        "expression",
        "old_classifier",
    ],
)
def test_archive_rejects_incomplete_or_misleading_license_metadata(damage):
    meta, prefix, contents = archive_license_fixture()
    key = prefix + "LICENSES/PolyForm-Noncommercial-1.0.0.md"
    if damage == "missing":
        del contents[key]
    elif damage == "stale":
        contents[key] += b"modified terms"
    elif damage == "wrong_directory":
        contents["unrelated/" + key] = contents.pop(key)
    elif damage == "duplicate":
        meta["License-File"] = "LICENSE"
    elif damage == "expression":
        meta.replace_header("License-Expression", "MIT")
    else:
        meta["Classifier"] = "License :: OSI Approved :: MIT License"
    with pytest.raises(ValueError):
        license_check.check_archive_licenses(contents.__getitem__, meta, prefix)


@pytest.mark.parametrize("path", license_check.PINNED_TEXTS)
def test_canonical_terms_and_legacy_copyright_cannot_drift(path):
    def read(name):
        data = (ROOT / name).read_bytes()
        return data.replace(b"License", b"Changed", 1) if name == path else data

    with pytest.raises(ValueError, match="Changed canonical license text"):
        license_check.check_pinned_texts(read)


@pytest.mark.parametrize(
    "damage", ["missing_dependency", "stale_checksum", "unsupported_exclusion"]
)
def test_dependency_catalog_requires_complete_reviewed_locked_sources(tmp_path, damage):
    shutil.copytree(ROOT / "LICENSES", tmp_path / "LICENSES")
    for path in license_check.LOCKFILES:
        (tmp_path / path).parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / path, tmp_path / path)
    path = tmp_path / "LICENSES/rust-dependencies.json"
    catalog = json.loads(path.read_bytes())
    if damage == "missing_dependency":
        catalog["packages"].pop()
    elif damage == "stale_checksum":
        catalog["packages"][0]["checksum"] = "0" * 64
    else:
        catalog["excluded"].append({**catalog["packages"].pop(), "reason": "skip"})
    path.write_text(json.dumps(catalog))
    with pytest.raises(ValueError):
        license_check.check_catalog(tmp_path)


def test_sdist_dependency_gate_rejects_old_version_and_wrong_registry_checksum():
    check = runpy.run_path(str(ROOT / "scripts/verify_release_artifacts.py"))[
        "_check_parasolid_dependency"
    ]
    workspace = (ROOT / "Cargo.toml").read_text()
    vendor = (ROOT / "vendor/cadmpeg-codec-sldprt/Cargo.toml").read_text()
    lock = (ROOT / "Cargo.lock").read_text()
    check(workspace, vendor, lock)
    with pytest.raises(AssertionError):
        check(
            workspace.replace('parasolid-core = "=0.2.0"', 'parasolid-core = "=0.1.0"'),
            vendor,
            lock,
        )
    expected = next(
        p
        for p in license_check.read_toml(ROOT / "Cargo.lock")["package"]
        if p["name"] == "parasolid-core"
    )
    with pytest.raises(AssertionError):
        check(workspace, vendor, lock.replace(expected["checksum"], "0" * 64))
