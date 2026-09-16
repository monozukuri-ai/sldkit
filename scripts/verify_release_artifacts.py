#!/usr/bin/env python3
from __future__ import annotations

import argparse
import email.parser
import sys
import tarfile
import zipfile
from pathlib import Path, PurePosixPath

import tomllib

sys.path.insert(0, str(Path(__file__).resolve().parent))
from check_license import (  # noqa: E402
    PARASOLID_VERSION,
    ROOT,
    check_archive_licenses,
    check_viewer_assets,
    notice_bundle,
    read_toml,
)

CAD_SUFFIXES = {".sldprt", ".sldasm", ".slddrw"}
FORBIDDEN_ROOTS = {"reference", "corpus", "designs", ".cargo", ".internal", "fuzz"}
FORBIDDEN_REQUIREMENTS = {
    "cad3d-ir",
    "cadquery",
    "occt",
    "pythonocc-core",
    "parasolid-kit",
}


def _expand_artifacts(paths: list[Path]) -> list[Path]:
    artifacts: list[Path] = []
    for path in paths:
        if path.is_dir():
            matches = sorted(
                child
                for child in path.iterdir()
                if child.suffix == ".whl" or child.name.endswith(".tar.gz")
            )
            if not matches:
                raise AssertionError(f"artifact directory is empty: {path}")
            artifacts.extend(matches)
        else:
            artifacts.append(path)
    return artifacts


def _normalized_parts(name: str) -> tuple[str, ...]:
    parts = PurePosixPath(name).parts
    if parts and parts[0].lower().startswith("sldkit-"):
        return parts[1:]
    return parts


def _check_names(path: Path, names: list[str]) -> None:
    failures = []
    for name in names:
        parts = _normalized_parts(name)
        if parts and parts[0].lower() in FORBIDDEN_ROOTS:
            failures.append(f"forbidden directory: {name}")
        if parts == ("preview.html",):
            failures.append(f"local viewer output: {name}")
        if PurePosixPath(name).suffix.lower() in CAD_SUFFIXES:
            failures.append(f"CAD binary: {name}")
    if failures:
        raise AssertionError(f"{path}: " + "; ".join(failures))


def _check_wheel(path: Path) -> None:
    assert "-cp310-abi3-" in path.name, f"wheel is not Python 3.10+ ABI3: {path}"
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        assert len(names) == len(set(names)), "duplicate archive paths"
        _check_names(path, names)
        assert any(name.endswith("/METADATA") for name in names), path
        assert any(name.endswith("/WHEEL") for name in names), path
        assert any(name.endswith("sldkit/py.typed") for name in names), path
        assert any(name.endswith("sldkit/_core.pyi") for name in names), path
        for asset in ("viewer.html", "viewer.css", "viewer.js", "three-LICENSE.txt"):
            assert f"sldkit/viewer/_assets/{asset}" in names, (path, asset)
        assert any(
            name.endswith("licenses/LICENSES/three-MIT.txt") for name in names
        ), path
        assert any(name.endswith("licenses/LICENSE") for name in names), path
        assert any(
            name.endswith("licenses/LICENSES/Apache-2.0.txt") for name in names
        ), path
        assert any(
            name.endswith("licenses/LICENSES/parasolid-core-MIT.txt") for name in names
        ), path
        assert any(
            "sldkit/_core" in name and name.endswith((".so", ".pyd", ".dylib"))
            for name in names
        ), path
        assert not any(
            name.endswith("scripts/capture_drawing_ground_truth.ps1")
            or name.endswith("scripts/compare_drawing_structures.py")
            for name in names
        ), path
        metadata_name = next(name for name in names if name.endswith("/METADATA"))
        metadata = email.parser.BytesParser().parsebytes(archive.read(metadata_name))
        assert sum(name.endswith("/METADATA") for name in names) == 1, path
        info = metadata_name.removesuffix("METADATA")
        check_archive_licenses(archive.read, metadata, info + "licenses/")
        check_viewer_assets(lambda name: archive.read("sldkit/viewer/_assets/" + name))
        wheel_name = next(name for name in names if name.endswith("/WHEEL"))
        wheel_metadata = email.parser.BytesParser().parsebytes(archive.read(wheel_name))

    assert metadata["Requires-Python"] == ">=3.10", path
    assert any(
        tag.startswith("cp310-abi3-") for tag in wheel_metadata.get_all("Tag", [])
    ), (path, wheel_metadata.get_all("Tag", []))
    requirements = {
        value.split(";", 1)[0].strip().split("[", 1)[0].split(" ", 1)[0].lower()
        for value in metadata.get_all("Requires-Dist", [])
    }
    assert requirements.isdisjoint(FORBIDDEN_REQUIREMENTS), (path, requirements)


def _check_sdist(path: Path) -> None:
    with tarfile.open(path, "r:gz") as archive:
        names = archive.getnames()
        assert len(names) == len(set(names)), "duplicate archive paths"

        def source(relative: str) -> str:
            name = next(
                name for name in names if "/".join(_normalized_parts(name)) == relative
            )
            member = archive.extractfile(name)
            assert member is not None, (path, relative)
            return member.read().decode("utf-8")

        def read(name: str) -> bytes:
            member = archive.extractfile(name)
            assert member is not None, name
            return member.read()

        pkg_info = next(
            name for name in names if _normalized_parts(name) == ("PKG-INFO",)
        )
        metadata = email.parser.BytesParser().parsebytes(read(pkg_info))
        check_archive_licenses(read, metadata, pkg_info.removesuffix("PKG-INFO"))
        check_viewer_assets(
            lambda name: source("python/sldkit/viewer/_assets/" + name).encode()
        )
        assert source("viewer/LICENSE.txt").encode() == notice_bundle(viewer=True)
        workspace = tomllib.loads(source("Cargo.toml"))["workspace"]
        for member in workspace["members"]:
            assert source(f"{member}/LICENSE").encode() == notice_bundle(), member
        _check_parasolid_dependency(
            source("Cargo.toml"),
            source("vendor/cadmpeg-codec-sldprt/Cargo.toml"),
            source("Cargo.lock"),
        )
        for name in (
            "topology",
            "native_fin",
            "native_hierarchy",
            "spline",
            "intersection",
            "blend",
            "offset",
            "sweep",
            "subset",
        ):
            adapter = source(f"vendor/cadmpeg-codec-sldprt/src/brep/{name}.rs")
            assert "parasolid_core::partial" in adapter, (path, name)
    _check_names(path, names)
    normalized = {"/".join(_normalized_parts(name)) for name in names}
    assert "Cargo.toml" in normalized, path
    assert "pyproject.toml" in normalized, path
    for required in (
        "CLA.md",
        "CONTRIBUTING.md",
        "docs/license.md",
        "docs/license.ja.md",
        "docs/releasing.md",
        "scripts/check_license.py",
        "scripts/sync_license_notices.py",
        "scripts/verify_release_artifacts.py",
        "scripts/verify_release_version.py",
    ):
        assert required in normalized, (path, required)
    assert "python/sldkit/__init__.py" in normalized, path
    assert "docs/README.md" in normalized, path
    assert "docs/architecture.md" in normalized, path
    assert "docs/compatibility.md" in normalized, path
    assert "docs/geometry.md" in normalized, path
    assert "docs/viewer.md" in normalized, path
    for asset in ("viewer.html", "viewer.css", "viewer.js", "three-LICENSE.txt"):
        assert f"python/sldkit/viewer/_assets/{asset}" in normalized, (path, asset)
    assert "viewer/src/viewer.js" in normalized, path
    assert "viewer/package-lock.json" in normalized, path
    assert "LICENSES/three-MIT.txt" in normalized, path
    assert "docs/drawing-structure.md" in normalized, path
    assert "docs/drawing-validation.md" in normalized, path
    assert "docs/schemas/drawing-ground-truth.schema.json" in normalized, path
    assert "docs/schemas/geometry-oracle.schema.json" in normalized, path
    assert "docs/schemas/nurbs-geometry-oracle.schema.json" in normalized, path
    assert "docs/schemas/nurbs-trim-oracle.schema.json" in normalized, path
    assert "docs/project-scanning.md" in normalized, path
    assert "docs/parser-provenance.md" in normalized, path
    assert "scripts/capture_drawing_ground_truth.ps1" in normalized, path
    assert "scripts/compare_drawing_structures.py" in normalized, path
    assert "scripts/capture_step_geometry.py" in normalized, path
    assert "scripts/nurbs_geometry.py" in normalized, path
    assert "scripts/validate_nurbs_geometry.py" in normalized, path
    assert "scripts/validate_nurbs_trim.py" in normalized, path
    assert "vendor/cadmpeg-codec-sldprt/src/brep/native_fin.rs" in normalized, path
    assert "LICENSE" in normalized, path
    assert "LICENSES/Apache-2.0.txt" in normalized, path
    assert "LICENSES/parasolid-core-MIT.txt" in normalized, path
    assert "LICENSES/README.md" in normalized, path
    for required in (
        "Cargo.toml",
        "LICENSE",
        "PATCHES.md",
        "src/brep/native_hierarchy.rs",
        "src/byte_ledger.rs",
    ):
        assert f"vendor/cadmpeg-codec-sldprt/{required}" in normalized, path


def _check_parasolid_dependency(
    workspace_source: str, vendor_source: str, lock_source: str
) -> None:
    workspace = tomllib.loads(workspace_source)["workspace"]
    vendor = tomllib.loads(vendor_source)
    assert workspace["dependencies"]["parasolid-core"] == "=" + PARASOLID_VERSION
    assert (
        vendor["dependencies"]["parasolid-core"]["version"] == "=" + PARASOLID_VERSION
    )
    assert vendor["package"]["license"] == "Apache-2.0"
    actual = [
        p
        for p in tomllib.loads(lock_source)["package"]
        if p["name"] == "parasolid-core"
    ]
    expected = [
        p
        for p in read_toml(ROOT / "Cargo.lock")["package"]
        if p["name"] == "parasolid-core"
    ]
    assert len(actual) == len(expected) == 1
    for field in ("version", "source", "checksum"):
        assert actual[0][field] == expected[0][field], (field, actual[0])


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("artifacts", nargs="+", type=Path)
    args = parser.parse_args()
    for artifact in _expand_artifacts(args.artifacts):
        if not artifact.is_file():
            raise AssertionError(f"artifact does not exist: {artifact}")
        if artifact.suffix == ".whl":
            _check_wheel(artifact)
        elif artifact.name.endswith(".tar.gz"):
            _check_sdist(artifact)
        else:
            raise AssertionError(f"unsupported artifact type: {artifact}")
        print(f"verified {artifact}")


if __name__ == "__main__":
    main()
