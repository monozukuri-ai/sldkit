#!/usr/bin/env python3
"""Update the shared version and refresh lockfiles (Python 3.11+, Cargo, uv)."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from verify_release_version import (  # noqa: E402
    ROOT,
    python_version,
    source_version,
    verify_release_version,
)

MANAGED_FILES = ("Cargo.toml", "Cargo.lock", "fuzz/Cargo.lock", "uv.lock")


def bump_version(
    version: str | None = None, root: Path = ROOT, *, offline: bool = False
) -> str:
    current = source_version(root)
    version = current if version is None else version
    python_version(version)  # Validate before writing or invoking package managers.
    original = {name: (root / name).read_bytes() for name in MANAGED_FILES}
    updated, count = re.subn(
        r"""(\[workspace\.package\]\s*\n(?:[^\[\n]*\n)*?version\s*=\s*)(["'])([^"']+)\2""",
        lambda match: f"{match[1]}{match[2]}{version}{match[2]}",
        original["Cargo.toml"].decode("utf-8"),
    )
    if count != 1:
        raise ValueError("expected one workspace.package.version in Cargo.toml")
    options = ["--offline"] if offline else []
    try:
        (root / "Cargo.toml").write_bytes(updated.encode("utf-8"))
        for command in (
            ["cargo", "update", "--workspace", *options],
            [
                "cargo",
                "update",
                "--workspace",
                "--manifest-path",
                "fuzz/Cargo.toml",
                *options,
            ],
            ["uv", "lock", *options],
        ):
            subprocess.run(command, cwd=root, check=True)
        verify_release_version(root)
    except BaseException:
        # Keep pre-existing edits too, including when a tool is interrupted.
        for name, content in original.items():
            (root / name).write_bytes(content)
        raise
    return version


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "version",
        nargs="?",
        help="X.Y.Z, X.Y.Z-rc.N or X.Y.Z-devN; omit to sync Cargo.toml",
    )
    parser.add_argument(
        "--offline", action="store_true", help="use cached dependencies"
    )
    parser.add_argument(
        "--check", action="store_true", help="verify without changing files"
    )
    parser.add_argument("--tag", help="expected release tag (with --check)")
    args = parser.parse_args()
    if args.check and args.version:
        parser.error("--check cannot be combined with a new version")
    if args.tag and not args.check:
        parser.error("--tag requires --check")
    try:
        if args.check:
            version = verify_release_version(tag=args.tag)
        else:
            version = bump_version(args.version, offline=args.offline)
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        parser.error(str(error))
    print(f"Rust {version}; Python {python_version(version)}; release tag v{version}")


if __name__ == "__main__":
    main()
