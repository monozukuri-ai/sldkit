#!/usr/bin/env python3
"""Compare two M6a Drawing inventories and optional SolidWorks API captures.

The report is deliberately evidential: XML records are matched by source fields
and exact record hashes, while candidate binary streams are compared only as
whole decoded payloads. It does not infer binary record framing or renderable
Drawing semantics.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import hashlib
import json
import sys
from collections import Counter, defaultdict
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path
from typing import Any


class DrawingDifferentialError(ValueError):
    """A Drawing inventory or API capture cannot be compared safely."""


JsonObject = Mapping[str, Any]
Signature = tuple[Any, ...]


def load_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8-sig"))


def _as_object(value: Any, field: str) -> JsonObject:
    if not isinstance(value, Mapping):
        raise DrawingDifferentialError(f"{field} must be an object")
    return value


def _drawing_structure(result: JsonObject, label: str) -> JsonObject:
    status = result.get("status")
    if status not in {"inventoried", "partial"}:
        raise DrawingDifferentialError(
            f"{label} Drawing status is {status!r}; expected inventoried or partial"
        )
    return _as_object(result.get("structure"), f"{label}.structure")


def _sorted_attributes(value: Any) -> tuple[tuple[str, str], ...]:
    attributes = _as_object(value or {}, "record.source_attributes")
    return tuple(sorted((str(name), str(item)) for name, item in attributes.items()))


def _record_logical_key(record: JsonObject) -> Signature:
    source = _as_object(record.get("source"), "record.source")
    return (
        str(source.get("stream_path")),
        str(record.get("class")),
        str(record.get("source_tag")),
        record.get("source_id"),
        record.get("name"),
        record.get("source_type"),
    )


def _record_content_signature(record: JsonObject) -> Signature:
    source = _as_object(record.get("source"), "record.source")
    return (
        *_record_logical_key(record),
        _sorted_attributes(record.get("source_attributes")),
        record.get("direct_text"),
        int(source.get("byte_len", 0)),
        str(source.get("sha256")),
    )


def _record_summary(record: JsonObject) -> dict[str, Any]:
    source = _as_object(record.get("source"), "record.source")
    return {
        "class": record.get("class"),
        "source_tag": record.get("source_tag"),
        "source_id": record.get("source_id"),
        "name": record.get("name"),
        "source_type": record.get("source_type"),
        "direct_text": record.get("direct_text"),
        "source_attributes": dict(
            sorted(
                _as_object(record.get("source_attributes") or {}, "attributes").items()
            )
        ),
        "source": {
            "stream_path": source.get("stream_path"),
            "decoded_offset": source.get("decoded_offset"),
            "byte_len": source.get("byte_len"),
            "sha256": source.get("sha256"),
        },
    }


def _json_sort_key(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def _counted_rows(
    counter: Counter[Signature],
    exemplars: Mapping[Signature, JsonObject],
    summarizer: Callable[[JsonObject], dict[str, Any]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for signature, count in counter.items():
        row = summarizer(exemplars[signature])
        row["occurrences"] = count
        rows.append(row)
    return sorted(rows, key=_json_sort_key)


def _compare_records(
    baseline: Sequence[JsonObject], variant: Sequence[JsonObject]
) -> dict[str, Any]:
    baseline_groups: defaultdict[Signature, list[JsonObject]] = defaultdict(list)
    variant_groups: defaultdict[Signature, list[JsonObject]] = defaultdict(list)
    for record in baseline:
        baseline_groups[_record_logical_key(record)].append(record)
    for record in variant:
        variant_groups[_record_logical_key(record)].append(record)

    changed: list[dict[str, Any]] = []
    removed: Counter[Signature] = Counter()
    added: Counter[Signature] = Counter()
    removed_examples: dict[Signature, JsonObject] = {}
    added_examples: dict[Signature, JsonObject] = {}
    preserved = 0

    keys = set(baseline_groups) | set(variant_groups)
    for key in sorted(keys, key=repr):
        before = baseline_groups[key]
        after = variant_groups[key]
        if len(before) == 1 and len(after) == 1:
            before_signature = _record_content_signature(before[0])
            after_signature = _record_content_signature(after[0])
            if before_signature == after_signature:
                preserved += 1
            else:
                changed.append(
                    {
                        "identity": {
                            "stream_path": key[0],
                            "class": key[1],
                            "source_tag": key[2],
                            "source_id": key[3],
                            "name": key[4],
                            "source_type": key[5],
                        },
                        "baseline": _record_summary(before[0]),
                        "variant": _record_summary(after[0]),
                    }
                )
            continue

        before_counter = Counter(_record_content_signature(record) for record in before)
        after_counter = Counter(_record_content_signature(record) for record in after)
        common = before_counter & after_counter
        preserved += sum(common.values())
        group_removed = before_counter - after_counter
        group_added = after_counter - before_counter
        removed.update(group_removed)
        added.update(group_added)
        for record in before:
            removed_examples.setdefault(_record_content_signature(record), record)
        for record in after:
            added_examples.setdefault(_record_content_signature(record), record)

    changed.sort(key=_json_sort_key)
    return {
        "content_preserved_count": preserved,
        "changed_count": len(changed),
        "removed_count": sum(removed.values()),
        "added_count": sum(added.values()),
        "changed": changed,
        "removed": _counted_rows(removed, removed_examples, _record_summary),
        "added": _counted_rows(added, added_examples, _record_summary),
    }


def _multiset_delta(
    baseline: Sequence[JsonObject],
    variant: Sequence[JsonObject],
    signature: Callable[[JsonObject], Signature],
    summarizer: Callable[[JsonObject], dict[str, Any]],
) -> dict[str, Any]:
    before_counter = Counter(signature(item) for item in baseline)
    after_counter = Counter(signature(item) for item in variant)
    before_examples = {signature(item): item for item in baseline}
    after_examples = {signature(item): item for item in variant}
    common = before_counter & after_counter
    removed = before_counter - after_counter
    added = after_counter - before_counter
    return {
        "preserved_count": sum(common.values()),
        "removed_count": sum(removed.values()),
        "added_count": sum(added.values()),
        "removed": _counted_rows(removed, before_examples, summarizer),
        "added": _counted_rows(added, after_examples, summarizer),
    }


def _sheet_key(sheet: JsonObject) -> Signature:
    return (sheet.get("source_id"), sheet.get("name"), sheet.get("source_type"))


def _sheet_summary(sheet: JsonObject) -> dict[str, Any]:
    return {
        "source_id": sheet.get("source_id"),
        "name": sheet.get("name"),
        "source_type": sheet.get("source_type"),
    }


def _record_identity_by_id(structure: JsonObject) -> dict[str, Signature]:
    return {
        str(record["id"]): _record_logical_key(record)
        for record in structure.get("records", [])
    }


def _record_hierarchy_items(structure: JsonObject) -> list[dict[str, Any]]:
    identities = _record_identity_by_id(structure)
    items: list[dict[str, Any]] = []
    for record in structure.get("records", []):
        parent_id = record.get("parent_id")
        items.append(
            {
                "record": record,
                "parent": (
                    identities.get(str(parent_id)) if parent_id is not None else None
                ),
            }
        )
    return items


def _record_hierarchy_key(item: JsonObject) -> Signature:
    record = _as_object(item.get("record"), "record hierarchy item")
    return (_record_logical_key(record), item.get("parent"))


def _record_hierarchy_summary(item: JsonObject) -> dict[str, Any]:
    record = _as_object(item.get("record"), "record hierarchy item")
    return {
        "record": {
            "stream_path": _record_logical_key(record)[0],
            "class": record.get("class"),
            "source_tag": record.get("source_tag"),
            "source_id": record.get("source_id"),
            "name": record.get("name"),
            "source_type": record.get("source_type"),
        },
        "parent": list(item["parent"]) if item.get("parent") is not None else None,
    }


def _sheet_identity_by_record_id(structure: JsonObject) -> dict[str, Signature]:
    return {
        str(sheet["record_id"]): _sheet_key(sheet)
        for sheet in structure.get("sheets", [])
    }


def _view_key(
    view: JsonObject,
    sheet_identities: Mapping[str, Signature],
    record_identities: Mapping[str, Signature],
) -> Signature:
    sheet_record_id = view.get("sheet_record_id")
    parent_record_id = view.get("parent_view_record_id")
    return (
        sheet_identities.get(str(sheet_record_id))
        if sheet_record_id is not None
        else None,
        view.get("source_id"),
        view.get("name"),
        view.get("referenced_document"),
        view.get("referenced_configuration"),
        record_identities.get(str(parent_record_id))
        if parent_record_id is not None
        else None,
    )


def _view_summary_factory(
    sheet_identities: Mapping[str, Signature],
    record_identities: Mapping[str, Signature],
) -> Callable[[JsonObject], dict[str, Any]]:
    def summarize(view: JsonObject) -> dict[str, Any]:
        key = _view_key(view, sheet_identities, record_identities)
        return {
            "sheet": list(key[0]) if key[0] is not None else None,
            "source_id": key[1],
            "name": key[2],
            "referenced_document": key[3],
            "referenced_configuration": key[4],
            "parent_view": list(key[5]) if key[5] is not None else None,
        }

    return summarize


def _carrier_key(carrier: JsonObject) -> Signature:
    return (str(carrier.get("stream_path")), str(carrier.get("role")))


def _carrier_content(carrier: JsonObject) -> Signature:
    return (
        int(carrier.get("decoded_size", 0)),
        str(carrier.get("decoded_sha256")),
        bool(carrier.get("record_framing_verified")),
    )


def _carrier_summary(carrier: JsonObject) -> dict[str, Any]:
    return {
        "stream_path": carrier.get("stream_path"),
        "role": carrier.get("role"),
        "decoded_size": carrier.get("decoded_size"),
        "decoded_sha256": carrier.get("decoded_sha256"),
        "record_framing_verified": carrier.get("record_framing_verified"),
    }


def _compare_carriers(
    baseline: Sequence[JsonObject], variant: Sequence[JsonObject]
) -> dict[str, Any]:
    before = {_carrier_key(item): item for item in baseline}
    after = {_carrier_key(item): item for item in variant}
    if len(before) != len(baseline) or len(after) != len(variant):
        raise DrawingDifferentialError(
            "candidate carrier path/role keys must be unique"
        )

    preserved = 0
    changed: list[dict[str, Any]] = []
    for key in sorted(set(before) & set(after), key=repr):
        if _carrier_content(before[key]) == _carrier_content(after[key]):
            preserved += 1
        else:
            changed.append(
                {
                    "stream_path": key[0],
                    "role": key[1],
                    "baseline": _carrier_summary(before[key]),
                    "variant": _carrier_summary(after[key]),
                }
            )
    removed = [_carrier_summary(before[key]) for key in set(before) - set(after)]
    added = [_carrier_summary(after[key]) for key in set(after) - set(before)]
    return {
        "preserved_count": preserved,
        "changed_count": len(changed),
        "removed_count": len(removed),
        "added_count": len(added),
        "changed": sorted(changed, key=_json_sort_key),
        "removed": sorted(removed, key=_json_sort_key),
        "added": sorted(added, key=_json_sort_key),
    }


def _source_snapshot(result: JsonObject, structure: JsonObject) -> dict[str, Any]:
    source = _as_object(structure.get("source"), "structure.source")
    coverage = _as_object(structure.get("coverage"), "structure.coverage")
    internal_version = structure.get("internal_version")
    version_value = (
        _as_object(internal_version, "structure.internal_version").get("value")
        if internal_version is not None
        else None
    )
    return {
        "status": result.get("status"),
        "source": {
            "byte_len": source.get("byte_len"),
            "sha256": source.get("sha256"),
        },
        "internal_version": version_value,
        "coverage": dict(coverage),
        "diagnostic_codes": [
            diagnostic.get("code") for diagnostic in result.get("diagnostics", [])
        ],
    }


def _api_view_key(sheet_name: str, view: JsonObject) -> Signature:
    return (sheet_name, view.get("name"))


def _api_view_summary(sheet_name: str, view: JsonObject) -> dict[str, Any]:
    result = {"sheet": sheet_name}
    result.update(dict(view))
    return result


def _api_sheet_summary(sheet: JsonObject) -> dict[str, Any]:
    return {
        "name": sheet.get("name"),
        "persistent_reference": sheet.get("persistent_reference"),
        "properties": sheet.get("properties"),
    }


def _numeric_vector(value: Any, field: str, count: int) -> list[int | float]:
    if not isinstance(value, Sequence) or isinstance(value, (str, bytes)):
        raise DrawingDifferentialError(f"{field} must be an array")
    if len(value) != count:
        raise DrawingDifferentialError(
            f"{field} must contain {count} values; found {len(value)}"
        )
    if not all(
        isinstance(item, (int, float)) and not isinstance(item, bool) for item in value
    ):
        raise DrawingDifferentialError(f"{field} values must be numbers")
    return list(value)


def _validate_persistent_reference(value: Any, field: str) -> None:
    reference = _as_object(value, field)
    status = reference.get("status")
    if status == "unavailable":
        for name in ("encoding", "byte_size", "sha256", "value"):
            if reference.get(name) is not None:
                raise DrawingDifferentialError(
                    f"{field}.{name} must be null when unavailable"
                )
        return
    if status != "captured":
        raise DrawingDifferentialError(
            f"{field}.status must be captured or unavailable"
        )
    if reference.get("encoding") != "base64":
        raise DrawingDifferentialError(f"{field}.encoding must be base64")
    encoded = reference.get("value")
    if not isinstance(encoded, str) or not encoded:
        raise DrawingDifferentialError(f"{field}.value must be non-empty base64")
    try:
        payload = base64.b64decode(encoded, validate=True)
    except (binascii.Error, ValueError) as error:
        raise DrawingDifferentialError(f"{field}.value is invalid base64") from error
    if not payload:
        raise DrawingDifferentialError(f"{field}.value decodes to no bytes")
    if reference.get("byte_size") != len(payload):
        raise DrawingDifferentialError(f"{field}.byte_size does not match value")
    if reference.get("sha256") != hashlib.sha256(payload).hexdigest():
        raise DrawingDifferentialError(f"{field}.sha256 does not match value")


def _validate_api_capture(
    capture: JsonObject, label: str
) -> tuple[JsonObject, JsonObject, list[JsonObject]]:
    if capture.get("schema_version") != 1:
        raise DrawingDifferentialError(f"{label} API schema_version must be 1")
    capture_tool = _as_object(capture.get("capture_tool"), f"{label}.capture_tool")
    if (
        capture_tool.get("name") != "capture_drawing_ground_truth.ps1"
        or capture_tool.get("version") != 1
    ):
        raise DrawingDifferentialError(f"{label} API capture tool is unsupported")
    source = _as_object(capture.get("source"), f"{label}.source")
    source_sha = source.get("sha256")
    if (
        not isinstance(source_sha, str)
        or len(source_sha) != 64
        or any(character not in "0123456789abcdef" for character in source_sha)
    ):
        raise DrawingDifferentialError(f"{label}.source.sha256 is invalid")
    if source.get("open_errors") != 0:
        raise DrawingDifferentialError(f"{label}.source.open_errors must be zero")
    drawing = _as_object(capture.get("drawing"), f"{label}.drawing")
    if drawing.get("length_unit") != "meter" or drawing.get("angle_unit") != "radian":
        raise DrawingDifferentialError(
            f"{label} API capture must use meter and radian units"
        )
    raw_sheets = drawing.get("sheets")
    if not isinstance(raw_sheets, Sequence) or isinstance(raw_sheets, (str, bytes)):
        raise DrawingDifferentialError(f"{label}.drawing.sheets must be an array")
    sheets = [
        _as_object(sheet, f"{label}.drawing.sheets[{index}]")
        for index, sheet in enumerate(raw_sheets)
    ]
    if not sheets:
        raise DrawingDifferentialError(f"{label}.drawing.sheets must not be empty")
    sheet_names = [sheet.get("name") for sheet in sheets]
    if any(not isinstance(name, str) or not name for name in sheet_names):
        raise DrawingDifferentialError(f"{label} API sheet names must be non-empty")
    if len(set(sheet_names)) != len(sheet_names):
        raise DrawingDifferentialError(f"{label} API sheet names must be unique")

    for sheet_index, sheet in enumerate(sheets):
        sheet_field = f"{label}.drawing.sheets[{sheet_index}]"
        _validate_persistent_reference(
            sheet.get("persistent_reference"),
            f"{sheet_field}.persistent_reference",
        )
        properties = _as_object(sheet.get("properties"), f"{sheet_field}.properties")
        _numeric_vector(
            properties.get("scale_ratio"), f"{sheet_field}.properties.scale_ratio", 2
        )
        raw_views = sheet.get("views")
        if not isinstance(raw_views, Sequence) or isinstance(raw_views, (str, bytes)):
            raise DrawingDifferentialError(f"{sheet_field}.views must be an array")
        view_names: list[str] = []
        for view_index, raw_view in enumerate(raw_views):
            view = _as_object(raw_view, f"{sheet_field}.views[{view_index}]")
            view_field = f"{sheet_field}.views[{view_index}]"
            name = view.get("name")
            if not isinstance(name, str) or not name:
                raise DrawingDifferentialError(f"{view_field}.name must be non-empty")
            view_names.append(name)
            _validate_persistent_reference(
                view.get("persistent_reference"),
                f"{view_field}.persistent_reference",
            )
            base_view = _as_object(view.get("base_view"), f"{view_field}.base_view")
            base_status = base_view.get("status")
            if base_status == "captured":
                if not isinstance(base_view.get("name"), str) or not base_view.get(
                    "name"
                ):
                    raise DrawingDifferentialError(
                        f"{view_field}.base_view.name must be non-empty"
                    )
                _validate_persistent_reference(
                    base_view.get("persistent_reference"),
                    f"{view_field}.base_view.persistent_reference",
                )
            elif base_status in {"none", "unavailable"}:
                if (
                    base_view.get("name") is not None
                    or base_view.get("persistent_reference") is not None
                ):
                    raise DrawingDifferentialError(
                        f"{view_field}.base_view unavailable fields must be null"
                    )
            else:
                raise DrawingDifferentialError(
                    f"{view_field}.base_view.status is invalid"
                )
            reference = _as_object(view.get("reference"), f"{view_field}.reference")
            resolution = reference.get("resolution_status")
            if resolution not in {
                "resolved",
                "missing",
                "ambiguous",
                "no_reference",
            }:
                raise DrawingDifferentialError(
                    f"{view_field}.reference.resolution_status is invalid"
                )
            if resolution == "no_reference" and (
                reference.get("stored_basename") is not None
                or reference.get("resolved_project_path") is not None
            ):
                raise DrawingDifferentialError(
                    f"{view_field}.reference no_reference paths must be null"
                )
            if (
                resolution == "resolved"
                and reference.get("resolved_project_path") is None
            ):
                raise DrawingDifferentialError(
                    f"{view_field}.reference resolved_project_path is required"
                )
            _numeric_vector(view.get("position_m"), f"{view_field}.position_m", 2)
            _numeric_vector(view.get("scale_ratio"), f"{view_field}.scale_ratio", 2)
            _numeric_vector(
                view.get("model_to_view_transform"),
                f"{view_field}.model_to_view_transform",
                13,
            )
        if len(set(view_names)) != len(view_names):
            raise DrawingDifferentialError(
                f"{sheet_field} API view names must be unique"
            )
    return source, drawing, sheets


def _compare_api_sheets(
    baseline: Sequence[JsonObject], variant: Sequence[JsonObject]
) -> dict[str, Any]:
    before = {str(sheet.get("name")): sheet for sheet in baseline}
    after = {str(sheet.get("name")): sheet for sheet in variant}
    changed: list[dict[str, Any]] = []
    preserved = 0
    for name in sorted(set(before) & set(after)):
        before_value = _api_sheet_summary(before[name])
        after_value = _api_sheet_summary(after[name])
        if before_value == after_value:
            preserved += 1
        else:
            changed.append(
                {
                    "name": name,
                    "baseline": before_value,
                    "variant": after_value,
                }
            )
    removed = [_api_sheet_summary(before[name]) for name in set(before) - set(after)]
    added = [_api_sheet_summary(after[name]) for name in set(after) - set(before)]
    return {
        "preserved_count": preserved,
        "changed_count": len(changed),
        "removed_count": len(removed),
        "added_count": len(added),
        "changed": sorted(changed, key=_json_sort_key),
        "removed": sorted(removed, key=_json_sort_key),
        "added": sorted(added, key=_json_sort_key),
    }


def _api_capture_delta(
    baseline: JsonObject,
    variant: JsonObject,
    baseline_sha256: str,
    variant_sha256: str,
    baseline_byte_len: int,
    variant_byte_len: int,
) -> dict[str, Any]:
    before_source, _, before_sheets = _validate_api_capture(baseline, "baseline")
    after_source, _, after_sheets = _validate_api_capture(variant, "variant")
    if before_source.get("sha256") != baseline_sha256:
        raise DrawingDifferentialError(
            "baseline API capture SHA-256 does not match the Drawing input"
        )
    if after_source.get("sha256") != variant_sha256:
        raise DrawingDifferentialError(
            "variant API capture SHA-256 does not match the Drawing input"
        )
    if before_source.get("byte_size") != baseline_byte_len:
        raise DrawingDifferentialError(
            "baseline API capture byte size does not match the Drawing input"
        )
    if after_source.get("byte_size") != variant_byte_len:
        raise DrawingDifferentialError(
            "variant API capture byte size does not match the Drawing input"
        )
    sheet_delta = _compare_api_sheets(before_sheets, after_sheets)
    before_views = [
        (str(sheet.get("name")), view)
        for sheet in before_sheets
        for view in sheet.get("views", [])
    ]
    after_views = [
        (str(sheet.get("name")), view)
        for sheet in after_sheets
        for view in sheet.get("views", [])
    ]
    before_by_key = {_api_view_key(sheet, view): view for sheet, view in before_views}
    after_by_key = {_api_view_key(sheet, view): view for sheet, view in after_views}
    if len(before_by_key) != len(before_views) or len(after_by_key) != len(after_views):
        raise DrawingDifferentialError("API view sheet/name keys must be unique")
    changed: list[dict[str, Any]] = []
    preserved = 0
    for key in sorted(set(before_by_key) & set(after_by_key), key=repr):
        before_value = _api_view_summary(key[0], before_by_key[key])
        after_value = _api_view_summary(key[0], after_by_key[key])
        if before_value == after_value:
            preserved += 1
        else:
            changed.append(
                {
                    "sheet": key[0],
                    "name": key[1],
                    "baseline": before_value,
                    "variant": after_value,
                }
            )
    removed = [
        _api_view_summary(key[0], before_by_key[key])
        for key in set(before_by_key) - set(after_by_key)
    ]
    added = [
        _api_view_summary(key[0], after_by_key[key])
        for key in set(after_by_key) - set(before_by_key)
    ]
    return {
        "same_solidworks_environment": (
            baseline.get("solidworks") == variant.get("solidworks")
        ),
        "baseline": {
            "fixture_id": baseline.get("fixture_id"),
            "solidworks": baseline.get("solidworks"),
            "source": dict(before_source),
        },
        "variant": {
            "fixture_id": variant.get("fixture_id"),
            "solidworks": variant.get("solidworks"),
            "source": dict(after_source),
        },
        "sheets": sheet_delta,
        "views": {
            "preserved_count": preserved,
            "changed_count": len(changed),
            "removed_count": len(removed),
            "added_count": len(added),
            "changed": sorted(changed, key=_json_sort_key),
            "removed": sorted(removed, key=_json_sort_key),
            "added": sorted(added, key=_json_sort_key),
        },
    }


def compare_results(
    baseline_result: JsonObject,
    variant_result: JsonObject,
    *,
    baseline_api: JsonObject | None = None,
    variant_api: JsonObject | None = None,
) -> dict[str, Any]:
    """Build a deterministic differential report from two Drawing results."""

    if (baseline_api is None) != (variant_api is None):
        raise DrawingDifferentialError(
            "baseline and variant API captures must be supplied together"
        )
    baseline_structure = _drawing_structure(baseline_result, "baseline")
    variant_structure = _drawing_structure(variant_result, "variant")
    baseline_records = list(baseline_structure.get("records", []))
    variant_records = list(variant_structure.get("records", []))
    baseline_sheets = list(baseline_structure.get("sheets", []))
    variant_sheets = list(variant_structure.get("sheets", []))
    baseline_views = list(baseline_structure.get("views", []))
    variant_views = list(variant_structure.get("views", []))
    baseline_record_ids = _record_identity_by_id(baseline_structure)
    variant_record_ids = _record_identity_by_id(variant_structure)
    baseline_sheet_ids = _sheet_identity_by_record_id(baseline_structure)
    variant_sheet_ids = _sheet_identity_by_record_id(variant_structure)

    baseline_snapshot = _source_snapshot(baseline_result, baseline_structure)
    variant_snapshot = _source_snapshot(variant_result, variant_structure)
    report: dict[str, Any] = {
        "schema_version": 1,
        "baseline": baseline_snapshot,
        "variant": variant_snapshot,
        "delta": {
            "source_changed": (
                baseline_snapshot["source"]["sha256"]
                != variant_snapshot["source"]["sha256"]
            ),
            "internal_version_changed": (
                baseline_snapshot["internal_version"]
                != variant_snapshot["internal_version"]
            ),
            "records": _compare_records(baseline_records, variant_records),
            "record_hierarchy": _multiset_delta(
                _record_hierarchy_items(baseline_structure),
                _record_hierarchy_items(variant_structure),
                _record_hierarchy_key,
                _record_hierarchy_summary,
            ),
            "sheets": _multiset_delta(
                baseline_sheets,
                variant_sheets,
                _sheet_key,
                _sheet_summary,
            ),
            "views": _compare_view_sets(
                baseline_views,
                variant_views,
                baseline_sheet_ids,
                variant_sheet_ids,
                baseline_record_ids,
                variant_record_ids,
            ),
            "candidate_streams": _compare_carriers(
                list(baseline_structure.get("source_streams", [])),
                list(variant_structure.get("source_streams", [])),
            ),
        },
        "interpretation": {
            "record_correspondence": "heuristic_source_fields_plus_exact_record_hash",
            "candidate_stream_granularity": "whole_decoded_stream",
            "binary_record_framing_verified": False,
            "api_to_native_record_mapping_verified": False,
            "renderable_semantics_verified": False,
        },
    }
    if baseline_api is not None and variant_api is not None:
        report["solidworks_api"] = _api_capture_delta(
            baseline_api,
            variant_api,
            str(baseline_snapshot["source"]["sha256"]),
            str(variant_snapshot["source"]["sha256"]),
            int(baseline_snapshot["source"]["byte_len"]),
            int(variant_snapshot["source"]["byte_len"]),
        )
    return report


def _compare_view_sets(
    baseline: Sequence[JsonObject],
    variant: Sequence[JsonObject],
    baseline_sheet_ids: Mapping[str, Signature],
    variant_sheet_ids: Mapping[str, Signature],
    baseline_record_ids: Mapping[str, Signature],
    variant_record_ids: Mapping[str, Signature],
) -> dict[str, Any]:
    before_counter = Counter(
        _view_key(view, baseline_sheet_ids, baseline_record_ids) for view in baseline
    )
    after_counter = Counter(
        _view_key(view, variant_sheet_ids, variant_record_ids) for view in variant
    )
    before_examples = {
        _view_key(view, baseline_sheet_ids, baseline_record_ids): view
        for view in baseline
    }
    after_examples = {
        _view_key(view, variant_sheet_ids, variant_record_ids): view for view in variant
    }
    common = before_counter & after_counter
    removed = before_counter - after_counter
    added = after_counter - before_counter
    return {
        "preserved_count": sum(common.values()),
        "removed_count": sum(removed.values()),
        "added_count": sum(added.values()),
        "removed": _counted_rows(
            removed,
            before_examples,
            _view_summary_factory(baseline_sheet_ids, baseline_record_ids),
        ),
        "added": _counted_rows(
            added,
            after_examples,
            _view_summary_factory(variant_sheet_ids, variant_record_ids),
        ),
    }


def decode_path(path: Path, profile: str) -> dict[str, Any]:
    import sldkit

    return sldkit.decode_drawing_structure_file(path, profile=profile).to_dict()


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Compare exact M6a Drawing inventory evidence"
    )
    parser.add_argument("baseline", type=Path)
    parser.add_argument("variant", type=Path)
    parser.add_argument("--profile", choices=("desktop", "service"), default="service")
    parser.add_argument("--baseline-api", type=Path)
    parser.add_argument("--variant-api", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    if (args.baseline_api is None) != (args.variant_api is None):
        parser.error("--baseline-api and --variant-api must be supplied together")

    try:
        report = compare_results(
            decode_path(args.baseline, args.profile),
            decode_path(args.variant, args.profile),
            baseline_api=(
                None if args.baseline_api is None else load_json(args.baseline_api)
            ),
            variant_api=(
                None if args.variant_api is None else load_json(args.variant_api)
            ),
        )
    except (DrawingDifferentialError, json.JSONDecodeError, OSError) as error:
        print(str(error), file=sys.stderr)
        return 2
    payload = json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.output is None:
        sys.stdout.write(payload)
    else:
        args.output.write_text(payload, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
