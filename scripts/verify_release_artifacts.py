#!/usr/bin/env python3
from __future__ import annotations

import argparse
import email.parser
import tarfile
import zipfile
from pathlib import Path, PurePosixPath

CAD_SUFFIXES = {".sldprt", ".sldasm", ".slddrw"}
FORBIDDEN_ROOTS = {"reference", "corpus", "designs"}
FORBIDDEN_REQUIREMENTS = {"cad3d-ir", "cadquery", "occt", "pythonocc-core"}


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
        if PurePosixPath(name).suffix.lower() in CAD_SUFFIXES:
            failures.append(f"CAD binary: {name}")
    if failures:
        raise AssertionError(f"{path}: " + "; ".join(failures))


def _check_wheel(path: Path) -> None:
    assert "-cp310-abi3-" in path.name, f"wheel is not Python 3.10+ ABI3: {path}"
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        _check_names(path, names)
        assert any(name.endswith("/METADATA") for name in names), path
        assert any(name.endswith("/WHEEL") for name in names), path
        assert any(name.endswith("sldkit/py.typed") for name in names), path
        assert any(name.endswith("sldkit/_core.pyi") for name in names), path
        assert any(name.endswith("licenses/LICENSE") for name in names), path
        assert any(
            name.endswith("licenses/LICENSES/Apache-2.0.txt") for name in names
        ), path
        assert any(
            "sldkit/_core" in name and name.endswith((".so", ".pyd", ".dylib"))
            for name in names
        ), path
        metadata_name = next(name for name in names if name.endswith("/METADATA"))
        metadata = email.parser.BytesParser().parsebytes(archive.read(metadata_name))
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
    _check_names(path, names)
    normalized = {"/".join(_normalized_parts(name)) for name in names}
    assert "Cargo.toml" in normalized, path
    assert "pyproject.toml" in normalized, path
    assert "python/sldkit/__init__.py" in normalized, path
    assert "docs/README.md" in normalized, path
    assert "docs/architecture.md" in normalized, path
    assert "docs/compatibility.md" in normalized, path
    assert "docs/geometry.md" in normalized, path
    assert "docs/schemas/geometry-oracle.schema.json" in normalized, path
    assert "docs/project-scanning.md" in normalized, path
    assert "docs/parser-provenance.md" in normalized, path
    assert "LICENSE" in normalized, path
    assert "LICENSES/Apache-2.0.txt" in normalized, path
    assert "LICENSES/README.md" in normalized, path


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
