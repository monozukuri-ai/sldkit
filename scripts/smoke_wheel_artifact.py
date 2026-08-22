#!/usr/bin/env python3
"""Install one wheel into a clean venv and run the installed-package smoke."""

from __future__ import annotations

import argparse
import os
import subprocess
import tempfile
import venv
from pathlib import Path

ROOT = Path(__file__).parents[1]
SMOKE_SCRIPT = ROOT / "scripts/smoke_installed_package.py"


def resolve_wheel(path: Path) -> Path:
    if path.is_file():
        if path.suffix != ".whl":
            raise ValueError(f"not a wheel: {path}")
        return path.resolve()
    if not path.is_dir():
        raise ValueError(f"wheel path does not exist: {path}")
    wheels = sorted(path.glob("*.whl"))
    if len(wheels) != 1:
        raise ValueError(
            f"expected exactly one wheel in {path}, found {len(wheels)}"
        )
    return wheels[0].resolve()


def environment_python(environment: Path) -> Path:
    if os.name == "nt":
        return environment / "Scripts/python.exe"
    return environment / "bin/python"


def smoke_wheel(wheel: Path) -> None:
    with tempfile.TemporaryDirectory(prefix="sldkit-wheel-smoke-") as directory:
        temporary_root = Path(directory)
        environment = temporary_root / "venv"
        venv.EnvBuilder(
            with_pip=True,
            clear=True,
            symlinks=os.name != "nt",
        ).create(environment)
        python = environment_python(environment)
        subprocess.run(
            [
                str(python),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-deps",
                "--no-index",
                str(wheel),
            ],
            cwd=temporary_root,
            check=True,
        )
        subprocess.run(
            [str(python), "-I", str(SMOKE_SCRIPT)],
            cwd=temporary_root,
            check=True,
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "wheel",
        type=Path,
        help="a wheel path or a directory containing exactly one wheel",
    )
    args = parser.parse_args()
    wheel = resolve_wheel(args.wheel)
    smoke_wheel(wheel)
    print(f"verified isolated wheel: {wheel.name}")


if __name__ == "__main__":
    main()
