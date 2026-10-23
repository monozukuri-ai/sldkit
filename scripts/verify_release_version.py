#!/usr/bin/env python3
"""Verify Cargo-owned release metadata and lockfiles (Python 3.11+)."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]


def read_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def python_version(version: str) -> str:
    """Convert the supported Cargo versions to Maturin's PEP 440 spelling."""
    number = r"(?:0|[1-9][0-9]*)"
    match = re.fullmatch(
        rf"({number}\.{number}\.{number})(?:-rc\.({number})|-dev({number}))?",
        version,
    )
    if not match:
        raise ValueError(
            f"unsupported release version: {version!r}; "
            "use X.Y.Z, X.Y.Z-rc.N or X.Y.Z-devN"
        )
    if match[2] is not None:
        return f"{match[1]}rc{match[2]}"
    if match[3] is not None:
        return f"{match[1]}.dev{match[3]}"
    return match[1]


def source_version(root: Path = ROOT) -> str:
    """Read the version without importing the installed native extension."""
    project = read_toml(root / "pyproject.toml")["project"]
    if "version" in project or "version" not in project.get("dynamic", []):
        raise ValueError("pyproject.toml must derive its version from Cargo.toml")
    version = read_toml(root / "Cargo.toml")["workspace"]["package"]["version"]
    python_version(version)
    return version


def _check_cargo_lock(root: Path, filename: str, expected: set[str], version: str):
    packages = read_toml(root / filename)["package"]
    for name in sorted(expected):
        matches = [p for p in packages if p["name"] == name and "source" not in p]
        if len(matches) != 1 or matches[0]["version"] != version:
            raise ValueError(f"{filename}: stale or missing {name} version")


def _fuzz_dependencies(root: Path, members: dict[str, dict]) -> set[str]:
    """Find workspace crates used by fuzz, including their local dependencies."""
    pending = [read_toml(root / "fuzz/Cargo.toml")]
    names: set[str] = set()
    while pending:
        manifest = pending.pop()
        for alias, dependency in manifest.get("dependencies", {}).items():
            name = (
                dependency.get("package", alias)
                if isinstance(dependency, dict)
                else alias
            )
            if name in members and name not in names:
                names.add(name)
                pending.append(members[name])
    return names


def verify_release_version(root: Path = ROOT, tag: str | None = None) -> str:
    version = source_version(root)
    workspace = read_toml(root / "Cargo.toml")["workspace"]
    members = {}
    for member in workspace["members"]:
        manifest = read_toml(root / member / "Cargo.toml")
        package = manifest["package"]
        if package.get("version") != {"workspace": True}:
            raise ValueError(f"{member} must inherit workspace.package.version")
        members[package["name"]] = manifest
    _check_cargo_lock(root, "Cargo.lock", set(members), version)
    _check_cargo_lock(
        root, "fuzz/Cargo.lock", _fuzz_dependencies(root, members), version
    )

    packages = [
        p for p in read_toml(root / "uv.lock")["package"] if p["name"] == "sldkit"
    ]
    if (
        len(packages) != 1
        or packages[0].get("source") != {"editable": "."}
        or "version" in packages[0]
    ):
        raise ValueError("uv.lock: sldkit must use dynamic local metadata; run uv lock")

    viewer = json.loads((root / "viewer/package.json").read_text(encoding="utf-8"))
    lock = json.loads((root / "viewer/package-lock.json").read_text(encoding="utf-8"))
    if viewer.get("private") is not True or any(
        "version" in package for package in (viewer, lock, lock["packages"][""])
    ):
        raise ValueError("viewer assets must remain private and unversioned")
    if tag and tag != f"v{version}":
        raise ValueError(f"release tag {tag!r} does not match version {version!r}")
    return version


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--tag", default="", help="expected Cargo tag, e.g. vX.Y.Z")
    args = parser.parse_args()
    try:
        version = verify_release_version(args.root, args.tag or None)
    except (ValueError, OSError, KeyError) as error:
        parser.error(str(error))
    print(f"verified release version: {version} (Python {python_version(version)})")


if __name__ == "__main__":
    main()
