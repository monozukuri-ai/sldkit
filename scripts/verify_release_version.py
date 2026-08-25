#!/usr/bin/env python3
"""Verify that every public version source agrees with the release tag."""

from __future__ import annotations

import argparse
import ast
import re
from pathlib import Path

ROOT = Path(__file__).parents[1]


def _toml_version(path: Path, section: str) -> str:
    active_section = False
    for line in path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped == f"[{section}]":
            active_section = True
            continue
        if stripped.startswith("["):
            active_section = False
            continue
        if not active_section:
            continue
        match = re.fullmatch(r"""version\s*=\s*(["'])([^"']+)\1\s*(?:#.*)?""", stripped)
        if match:
            return match.group(2)
    raise ValueError(f"missing {section}.version in {path}")


def _python_version(path: Path) -> str:
    module = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    for statement in module.body:
        if not isinstance(statement, (ast.Assign, ast.AnnAssign)):
            continue
        targets = (
            statement.targets
            if isinstance(statement, ast.Assign)
            else [statement.target]
        )
        if not any(
            isinstance(target, ast.Name) and target.id == "__version__"
            for target in targets
        ):
            continue
        value = statement.value
        if isinstance(value, ast.Constant) and isinstance(value.value, str):
            return value.value
        raise ValueError(f"__version__ must be a string literal in {path}")
    raise ValueError(f"missing __version__ in {path}")


def release_versions(root: Path) -> dict[str, str]:
    return {
        "pyproject.toml": _toml_version(root / "pyproject.toml", "project"),
        "Cargo.toml": _toml_version(root / "Cargo.toml", "workspace.package"),
        "python/sldkit/__init__.py": _python_version(
            root / "python/sldkit/__init__.py"
        ),
    }


def verify_release_version(root: Path, tag: str | None = None) -> str:
    versions = release_versions(root)
    unique_versions = set(versions.values())
    if len(unique_versions) != 1:
        details = ", ".join(
            f"{source}={version}" for source, version in versions.items()
        )
        raise ValueError(f"release versions differ: {details}")

    version = unique_versions.pop()
    if tag and tag != f"v{version}":
        raise ValueError(f"release tag {tag!r} does not match version {version!r}")
    return version


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument(
        "--tag",
        default="",
        help="tag to validate; an empty value only checks source version alignment",
    )
    args = parser.parse_args()
    try:
        version = verify_release_version(args.root, args.tag or None)
    except ValueError as error:
        parser.error(str(error))
    print(f"verified release version: {version}")


if __name__ == "__main__":
    main()
