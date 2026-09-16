"""Generate crate/viewer notices; optionally refresh cached registry attribution."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path

from check_license import (
    EXCLUSIONS,
    LOCKFILES,
    ROOT,
    check_pinned_texts,
    locked_packages,
    notice_bundle,
    read_toml,
    sha256,
)

MIT_OPTIONS = {
    "MIT",
    "MIT OR Apache-2.0",
    "Apache-2.0 OR MIT",
    "Apache-2.0 / MIT",
    "MIT/Apache-2.0",
    "Unlicense OR MIT",
    "MIT OR Zlib OR Apache-2.0",
    "0BSD OR MIT OR Apache-2.0",
    "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
}
SELECTIONS = {
    **dict.fromkeys(MIT_OPTIONS, "MIT"),
    "MIT AND Apache-2.0": "MIT AND Apache-2.0",
    "Apache-2.0": "Apache-2.0",
    "(MIT OR Apache-2.0) AND Unicode-3.0": "MIT AND Unicode-3.0",
    "Apache-2.0 WITH LLVM-exception": "Apache-2.0 WITH LLVM-exception",
}
CADMPEG_SOURCE = (
    "https://github.com/cadmpeg/cadmpeg/blob/"
    "dbb308e52ab05c911b26963a02fbb294eefab3ce/LICENSE"
)


def refresh_rust(root: Path = ROOT) -> None:
    metadata = {}
    for lock in LOCKFILES:
        manifest = str(Path(lock).with_name("Cargo.toml"))
        result = subprocess.check_output(
            [
                "cargo",
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
                "--manifest-path",
                manifest,
            ],
            cwd=root,
        )
        for package in json.loads(result)["packages"]:
            metadata[package["name"], package["version"], package["source"]] = package
    registries: dict[str, set[Path]] = {}
    for package in metadata.values():
        if package["source"]:
            registries.setdefault(package["source"], set()).add(
                Path(package["manifest_path"]).parent.parent
            )
    packages, excluded, sections = [], [], []
    for key, locked in sorted(locked_packages(root).items()):
        name, version, source = key
        row = {
            k: locked[k] for k in ("name", "version", "source", "checksum", "lockfiles")
        }
        package = metadata.get(key)
        if package is None:
            # Cargo.lock can retain optional packages outside the resolved graph.
            # Read their normalized registry manifest, verifying the archive checksum.
            candidates = [base / f"{name}-{version}" for base in registries[source]]
            candidates = [
                base for base in candidates if (base / "Cargo.toml").is_file()
            ]
            if len(candidates) != 1:
                raise ValueError(
                    f"Fetch locked registry source before refreshing: {key}"
                )
            base = candidates[0]
            archive = (
                base.parents[2] / "cache" / base.parent.name / f"{name}-{version}.crate"
            )
            if sha256(archive.read_bytes()) != row["checksum"]:
                raise ValueError(f"Registry source checksum mismatch: {key}")
            package = {
                **read_toml(base / "Cargo.toml")["package"],
                "manifest_path": str(base / "Cargo.toml"),
            }
        row["upstream_license"] = package["license"]
        if (name, version) in EXCLUSIONS:
            if name == "libfuzzer-sys" and row["lockfiles"] != ["fuzz/Cargo.lock"]:
                raise ValueError("libFuzzer is no longer fuzz-only")
            excluded.append({**row, "reason": EXCLUSIONS[name, version]})
            continue
        if package["license"] not in SELECTIONS:
            raise ValueError(
                f"Unreviewed dependency license: {name}: {package['license']}"
            )
        selected = SELECTIONS[package["license"]]
        base = Path(package["manifest_path"]).parent
        candidates = [
            f
            for f in sorted(base.iterdir())
            if f.is_file()
            and f.name.upper().startswith(("LICENSE", "COPYING", "COPYRIGHT", "NOTICE"))
        ]
        origin = "registry package"
        if not candidates and name in {
            "cadmpeg-container",
            "cadmpeg-core",
            "cadmpeg-ir",
        }:
            if version != "0.5.3":
                raise ValueError("Review upstream notice for new cadmpeg version")
            base = root
            candidates = [root / "LICENSES/Apache-2.0.txt"]
            origin = CADMPEG_SOURCE + " (registry package omits license text)"
        if not candidates:
            raise ValueError(f"Missing dependency license text: {name} {version}")
        mit = [f for f in candidates if "MIT" in f.name.upper()]
        if mit and selected.startswith("MIT") and "AND Apache-2.0" not in selected:
            candidates = [
                f
                for f in candidates
                if f in mit
                or f.name.upper() == "LICENSE"
                or f.name.upper().startswith(("COPYRIGHT", "NOTICE", "COPYING"))
                or "UNICODE" in f.name.upper()
            ]
        if name == "zstd-sys":
            candidates.append(base / "zstd/LICENSE")
            selected += " AND BSD-3-Clause"
        entries = []
        for f in sorted(set(candidates)):
            data = f.read_bytes()
            relative = f.relative_to(base).as_posix()
            entries.append({"path": relative, "sha256": sha256(data)})
            sections.append(
                f"===== {name} {version}: {relative} =====\n\n".encode() + data + b"\n"
            )
        packages.append(
            {
                **row,
                "selected_license": selected,
                "license_source": origin,
                "license_files": entries,
            }
        )
    text = (
        b"Locked Cargo dependency notices, including build, development "
        b"and target-specific tools.\n"
        b"Generated by scripts/sync_license_notices.py --refresh-rust.\n"
        b"Selected terms, provenance and documented exclusions "
        b"are in rust-dependencies.json.\n\n" + b"\n".join(sections)
    )
    (root / "LICENSES/rust-dependencies.txt").write_bytes(text)
    catalog = {
        "schema_version": 1,
        "notices_sha256": sha256(text),
        "packages": packages,
        "excluded": excluded,
    }
    (root / "LICENSES/rust-dependencies.json").write_bytes(
        (json.dumps(catalog, indent=2) + "\n").encode()
    )


def sync(root: Path = ROOT) -> None:
    check_pinned_texts(lambda p: (root / p).read_bytes())
    for member in [*read_toml(root / "Cargo.toml")["workspace"]["members"], "fuzz"]:
        (root / member / "LICENSE").write_bytes(notice_bundle(root))
    viewer = notice_bundle(root, viewer=True)
    if b"*/" in viewer:
        raise ValueError("Viewer notice cannot contain a JavaScript comment terminator")
    (root / "viewer/LICENSE.txt").write_bytes(viewer)
    (root / "python/sldkit/viewer/_assets/LICENSE.txt").write_bytes(viewer)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--refresh-rust",
        action="store_true",
        help="Read cached registry sources for all three locked workspaces",
    )
    args = parser.parse_args()
    if args.refresh_rust:
        refresh_rust()
    sync()
    print("Updated crate and viewer notices; rebuild viewer assets next")


if __name__ == "__main__":
    main()
