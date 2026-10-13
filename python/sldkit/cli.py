from __future__ import annotations

import argparse
import json
import sys
from collections.abc import Sequence
from pathlib import Path

from .api import (
    build_bom,
    decode_drawing_structure_file,
    decode_geometry_file,
    evaluate_geometry_file,
    extract_file,
    index_project,
    inspect_file,
    parse_file,
    probe_file,
    scan_project,
)
from .model import (
    BomStatus,
    DrawingStructureStatus,
    ExtractionStatus,
    GeometryEvaluationStatus,
    GeometryStatus,
    InventoryStatus,
    ParseStatus,
    ProbeStatus,
    ProjectScanStatus,
)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="sldkit", description="Inspect and decode supported SolidWorks source data"
    )
    subparsers = parser.add_subparsers(dest="command", required=True)
    for name in ("probe", "inspect", "parse", "geometry", "drawing"):
        command = subparsers.add_parser(name)
        command.add_argument("path")
        command.add_argument(
            "--limits", choices=("desktop", "service"), default="desktop"
        )
    evaluate = subparsers.add_parser(
        "evaluate", help="Decode Part geometry and evaluate edges, bounds, and meshes"
    )
    evaluate.add_argument("path")
    evaluate.add_argument("--chord-tolerance", type=float, default=0.01)
    evaluate.add_argument("--max-segments-per-edge", type=int, default=512)
    evaluate.add_argument("--tangent-angle", type=float, default=0.5)
    evaluate.add_argument("--mesh-feature-angle", type=float, default=30.0)
    evaluate.add_argument("--configuration")
    evaluate.add_argument("--limits", choices=("desktop", "service"), default="desktop")
    extract = subparsers.add_parser("extract")
    extract.add_argument("path")
    extract.add_argument("entry_id")
    extract.add_argument("output")
    extract.add_argument("--mode", choices=("stored", "decoded"), default="decoded")
    extract.add_argument("--limits", choices=("desktop", "service"), default="desktop")
    extract.add_argument("--force", action="store_true")
    view = subparsers.add_parser("view", help="Write an offline HTML viewer")
    view.add_argument("path")
    view.add_argument(
        "--output", "-o", help="HTML path (default: input with .html suffix)"
    )
    view.add_argument("--limits", choices=("desktop", "service"), default="desktop")
    view.add_argument("--force", action="store_true", help="Replace an existing HTML")
    view.add_argument("--open", action="store_true", help="Open the generated HTML")
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
    index = subparsers.add_parser(
        "index", help="Parse every document under a project root"
    )
    index.add_argument("project_root")
    _add_resolution_arguments(index)
    index.add_argument("--summary", action="store_true")
    index.add_argument("--limits", choices=("desktop", "service"), default="desktop")
    bom = subparsers.add_parser(
        "bom", help="Expand an assembly into a bill of materials"
    )
    bom.add_argument("path")
    bom.add_argument("--project-root")
    bom.add_argument("--configuration")
    _add_resolution_arguments(bom)
    bom.add_argument("--include-suppressed", action="store_true")
    bom.add_argument("--include-excluded", action="store_true")
    bom.add_argument("--items-only", action="store_true")
    bom.add_argument("--limits", choices=("desktop", "service"), default="desktop")
    return parser


def _add_resolution_arguments(command: argparse.ArgumentParser) -> None:
    command.add_argument("--search-dir", action="append", default=[])
    command.add_argument(
        "--windows-prefix-map",
        action="append",
        default=[],
        type=_windows_prefix_mapping,
        metavar="SOURCE=TARGET",
    )
    command.add_argument("--follow-suppressed", action="store_true")


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "view":
        from .viewer import view_file

        try:
            output = view_file(
                args.path, args.output, profile=args.limits, force=args.force
            )
        except (OSError, ValueError) as error:
            print(f"sldkit view: {error}", file=sys.stderr)
            return 2
        print(output)
        if args.open:
            import webbrowser

            if not webbrowser.open(output.resolve().as_uri()):
                print(
                    "Could not open a browser; open the HTML manually.", file=sys.stderr
                )
        return 0
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

    if args.command == "evaluate":
        evaluation = evaluate_geometry_file(
            args.path,
            profile=args.limits,
            chord_tolerance_mm=args.chord_tolerance,
            max_segments_per_edge=args.max_segments_per_edge,
            tangent_angle_deg=args.tangent_angle,
            mesh_feature_angle_deg=args.mesh_feature_angle,
            configuration=args.configuration,
        )
        print(json.dumps(evaluation.to_dict(), indent=2, sort_keys=True))
        if evaluation.status in {
            GeometryEvaluationStatus.EVALUATED,
            GeometryEvaluationStatus.PARTIAL,
        }:
            return 0
        return 1 if evaluation.status is GeometryEvaluationStatus.UNAVAILABLE else 2

    if args.command == "index":
        result = index_project(
            args.project_root,
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

    if args.command == "bom":
        graph = scan_project(
            args.path,
            project_root=args.project_root,
            configuration=args.configuration,
            search_directories=args.search_dir,
            windows_prefix_mappings=args.windows_prefix_map,
            follow_suppressed=args.follow_suppressed,
            profile=args.limits,
        )
        bom = build_bom(
            graph,
            root_configuration=args.configuration,
            include_suppressed=args.include_suppressed,
            include_excluded=args.include_excluded,
        )
        payload = bom.to_dict()
        if args.items_only:
            payload["occurrences"] = []
        print(json.dumps(payload, indent=2, sort_keys=True))
        if bom.status is BomStatus.COMPLETE:
            return 0
        return 1 if bom.status is BomStatus.PARTIAL else 2

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

    if args.command == "geometry":
        result = decode_geometry_file(args.path, profile=args.limits)
        print(json.dumps(result.to_dict(), indent=2, sort_keys=True))
        if result.status in {GeometryStatus.DECODED, GeometryStatus.PARTIAL}:
            return 0
        return 1 if result.status is GeometryStatus.UNSUPPORTED else 2

    if args.command == "drawing":
        result = decode_drawing_structure_file(args.path, profile=args.limits)
        print(json.dumps(result.to_dict(), indent=2, sort_keys=True))
        if result.status in {
            DrawingStructureStatus.INVENTORIED,
            DrawingStructureStatus.PARTIAL,
        }:
            return 0
        return 1 if result.status is DrawingStructureStatus.UNSUPPORTED else 2

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
