from __future__ import annotations

import argparse
import json
from collections.abc import Sequence
from pathlib import Path

from .api import extract_file, inspect_file, parse_file, probe_file, scan_project
from .model import (
    ExtractionStatus,
    InventoryStatus,
    ParseStatus,
    ProbeStatus,
    ProjectScanStatus,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="sldkit", description="Inspect SolidWorks containers and source metadata"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("probe", "inspect", "parse"):
        command = subparsers.add_parser(name)
        command.add_argument("path")
        command.add_argument(
            "--limits", choices=("desktop", "service"), default="desktop"
        )
    extract = subparsers.add_parser("extract")
    extract.add_argument("path")
    extract.add_argument("entry_id")
    extract.add_argument("output")
    extract.add_argument("--mode", choices=("stored", "decoded"), default="decoded")
    extract.add_argument("--limits", choices=("desktop", "service"), default="desktop")
    extract.add_argument("--force", action="store_true")
    scan = subparsers.add_parser("scan")
    scan.add_argument("path")
    scan.add_argument("--project-root")
    scan.add_argument("--configuration")
    scan.add_argument("--search-dir", action="append", default=[])
    scan.add_argument(
        "--windows-prefix-map",
        action="append",
        default=[],
        type=_windows_prefix_mapping,
        metavar="SOURCE=TARGET",
    )
    scan.add_argument("--follow-suppressed", action="store_true")
    scan.add_argument("--summary", action="store_true")
    scan.add_argument("--limits", choices=("desktop", "service"), default="desktop")
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "probe":
        result = probe_file(args.path, profile=args.limits)
        print(json.dumps(result.to_dict(), indent=2, sort_keys=True))
        return (
            0
            if result.status is ProbeStatus.RECOGNIZED
            else 2
            if result.status
            in {
                ProbeStatus.MALFORMED,
                ProbeStatus.REJECTED,
            }
            else 1
        )

    if args.command == "inspect":
        result = inspect_file(args.path, profile=args.limits)
        print(json.dumps(result.to_dict(), indent=2, sort_keys=True))
        if result.status in {InventoryStatus.COMPLETE, InventoryStatus.PARTIAL}:
            return 0
        return 1 if result.status is InventoryStatus.UNSUPPORTED else 2

    if args.command == "extract":
        extraction = extract_file(
            args.path,
            args.entry_id,
            mode=args.mode,
            profile=args.limits,
        )
        print(json.dumps(extraction.result.to_dict(), indent=2, sort_keys=True))
        if extraction.result.status is ExtractionStatus.EXTRACTED:
            if extraction.data is not None:
                with Path(args.output).open("wb" if args.force else "xb") as output:
                    output.write(extraction.data)
            return 0
        if extraction.result.status in {
            ExtractionStatus.NOT_FOUND,
            ExtractionStatus.UNAVAILABLE,
        }:
            return 1
        return 2

    if args.command == "scan":
        result = scan_project(
            args.path,
            project_root=args.project_root,
            configuration=args.configuration,
            search_directories=args.search_dir,
            windows_prefix_mappings=args.windows_prefix_map,
            follow_suppressed=args.follow_suppressed,
            profile=args.limits,
        )
        output = result.compatibility_report if args.summary else result
        print(json.dumps(output.to_dict(), indent=2, sort_keys=True))
        if result.status is ProjectScanStatus.COMPLETE:
            return 0
        return 1 if result.status is ProjectScanStatus.PARTIAL else 2

    result = parse_file(args.path, profile=args.limits)
    print(json.dumps(result.to_dict(), indent=2, sort_keys=True))
    if result.status in {ParseStatus.PARSED, ParseStatus.PARTIAL}:
        return 0
    return 1 if result.status is ParseStatus.UNSUPPORTED else 2


def _windows_prefix_mapping(value: str) -> tuple[str, str]:
    try:
        source, target = value.split("=", 1)
    except ValueError as error:
        raise argparse.ArgumentTypeError("expected SOURCE=TARGET") from error
    if not source or not target:
        raise argparse.ArgumentTypeError("SOURCE and TARGET must be non-empty")
    return source, target
