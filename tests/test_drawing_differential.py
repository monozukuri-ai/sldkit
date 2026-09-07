from __future__ import annotations

import copy
import hashlib
import json
import runpy
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).parents[1]
SCHEMA = ROOT / "docs/schemas/drawing-ground-truth.schema.json"


def _record(
    record_id: str,
    record_class: str,
    tag: str,
    sha: str,
    *,
    parent_id: str | None = None,
    source_id: str | None = None,
    name: str | None = None,
    direct_text: str | None = None,
) -> dict:
    return {
        "id": record_id,
        "class": record_class,
        "source_tag": tag,
        "parent_id": parent_id,
        "source_id": source_id,
        "name": name,
        "source_type": None,
        "source_attributes": {},
        "direct_text": direct_text,
        "source": {
            "entry_id": "modern:keywords",
            "stream_path": "swXmlContents/KeyWords",
            "decoded_offset": 0 if parent_id is None else 10,
            "byte_len": 10,
            "sha256": sha,
        },
    }


def _result(source_sha: str, view_reference: str = "part.SLDPRT") -> dict:
    root = _record("root", "root", "Root", "1" * 64)
    sheet = _record(
        "sheet",
        "sheet",
        "Sheet",
        "2" * 64,
        parent_id="root",
        source_id="sheet-1",
        name="Sheet1",
    )
    view = _record(
        "view",
        "view",
        "View",
        "3" * 64,
        parent_id="sheet",
        source_id="view-1",
        name="Drawing View1",
        direct_text=view_reference,
    )
    return {
        "status": "partial",
        "structure": {
            "source": {
                "input_kind": "bytes",
                "label": "fixture.SLDDRW",
                "byte_len": 100,
                "sha256": source_sha,
            },
            "internal_version": {
                "value": 19000,
                "origin": "source",
            },
            "records": [root, sheet, view],
            "sheets": [
                {
                    "record_id": "sheet",
                    "source_id": "sheet-1",
                    "name": "Sheet1",
                    "source_type": None,
                    "view_record_ids": ["view"],
                }
            ],
            "views": [
                {
                    "record_id": "view",
                    "sheet_record_id": "sheet",
                    "source_id": "view-1",
                    "name": "Drawing View1",
                    "referenced_document": view_reference,
                    "referenced_configuration": "Default",
                    "parent_view_record_id": None,
                }
            ],
            "source_streams": [
                {
                    "entry_id": "modern:definition",
                    "stream_path": "Contents/Definition",
                    "role": "definition_candidate",
                    "decoded_size": 20,
                    "decoded_sha256": "4" * 64,
                    "record_framing_verified": False,
                }
            ],
            "coverage": {
                "record_count": 3,
                "record_class_counts": {"root": 1, "sheet": 1, "view": 1},
                "sheet_record_count": 1,
                "supported_sheet_count": 1,
                "sheet_view_count": 1,
                "unassigned_view_record_count": 0,
                "candidate_stream_count": 1,
                "candidate_stream_bytes": 20,
                "located_record_count": 3,
                "unique_record_range_count": 3,
                "partition_status": "incomplete",
                "typed_bytes": None,
                "uninterpreted_bytes": None,
            },
        },
        "diagnostics": [
            {
                "code": "drawing.byte_partition_incomplete",
                "severity": "warning",
                "kind": "preserved",
                "message": "test",
                "offset": None,
                "stream_path": None,
            }
        ],
    }


def _persistent_reference() -> dict:
    value = b"\x01\x02\x03"
    return {
        "status": "captured",
        "encoding": "base64",
        "byte_size": len(value),
        "sha256": hashlib.sha256(value).hexdigest(),
        "value": "AQID",
    }


def _api_capture(source_sha: str, path: str, x_position: float = 0.1) -> dict:
    return {
        "schema_version": 1,
        "fixture_id": f"capture-{path}",
        "captured_at_utc": "2026-08-26T00:00:00Z",
        "capture_tool": {
            "name": "capture_drawing_ground_truth.ps1",
            "version": 1,
            "powershell_version": "5.1",
        },
        "solidworks": {
            "revision_number": "34.0.0",
            "base_version": "SW2026",
            "build_number": "test",
            "hot_fixes": "",
            "current_license_type": {"code": 0, "name": "full"},
        },
        "source": {
            "path": path,
            "sha256": source_sha,
            "byte_size": 100,
            "open_errors": 0,
            "open_warnings": 0,
            "saved_license_type": {"code": 0, "name": "full"},
        },
        "drawing": {
            "length_unit": "meter",
            "angle_unit": "radian",
            "sheets": [
                {
                    "name": "Sheet1",
                    "persistent_reference": _persistent_reference(),
                    "properties": {
                        "paper_size_code": 7,
                        "template_code": 12,
                        "scale_ratio": [1.0, 1.0],
                        "first_angle_projection": False,
                        "width_m": 0.297,
                        "height_m": 0.21,
                        "same_custom_properties": False,
                    },
                    "views": [
                        {
                            "name": "Drawing View1",
                            "persistent_reference": _persistent_reference(),
                            "view_type": {"code": 6, "name": "standard"},
                            "base_view": {
                                "status": "none",
                                "name": None,
                                "persistent_reference": None,
                            },
                            "reference": {
                                "stored_basename": "part.SLDPRT",
                                "resolved_project_path": "part.SLDPRT",
                                "resolution_status": "resolved",
                            },
                            "referenced_configuration": "Default",
                            "position_m": [x_position, 0.1],
                            "scale_decimal": 1.0,
                            "scale_ratio": [1.0, 1.0],
                            "use_sheet_scale": True,
                            "use_parent_scale": False,
                            "angle_rad": 0.0,
                            "model_to_view_transform": [0.0] * 13,
                        }
                    ],
                }
            ],
        },
    }


def _namespace() -> dict:
    return runpy.run_path(str(ROOT / "scripts/compare_drawing_structures.py"))


def _schema_validator() -> Draft202012Validator:
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator(schema, format_checker=FormatChecker())


def test_drawing_ground_truth_schema_accepts_complete_capture():
    errors = list(
        _schema_validator().iter_errors(_api_capture("a" * 64, "base.SLDDRW"))
    )
    assert errors == []


def test_drawing_ground_truth_schema_preserves_unavailable_and_no_reference():
    capture = _api_capture("a" * 64, "base.SLDDRW")
    unavailable = {
        "status": "unavailable",
        "encoding": None,
        "byte_size": None,
        "sha256": None,
        "value": None,
    }
    sheet = capture["drawing"]["sheets"][0]
    sheet["persistent_reference"] = unavailable
    view = sheet["views"][0]
    view["persistent_reference"] = unavailable
    view["base_view"] = {
        "status": "unavailable",
        "name": None,
        "persistent_reference": None,
    }
    view["reference"] = {
        "stored_basename": None,
        "resolved_project_path": None,
        "resolution_status": "no_reference",
    }
    view["referenced_configuration"] = None

    assert list(_schema_validator().iter_errors(capture)) == []


def test_drawing_ground_truth_schema_rejects_absolute_source_path():
    capture = _api_capture("a" * 64, "C:/private/base.SLDDRW")

    assert list(_schema_validator().iter_errors(capture))


def test_differential_reports_record_carrier_view_and_api_changes():
    namespace = _namespace()
    baseline = _result("a" * 64)
    variant = _result("b" * 64, "variant.SLDPRT")
    variant["structure"]["records"][2]["source"]["sha256"] = "5" * 64
    variant["structure"]["records"][2]["parent_id"] = "root"
    variant["structure"]["source_streams"][0]["decoded_sha256"] = "6" * 64
    baseline_api = _api_capture("a" * 64, "base.SLDDRW")
    variant_api = _api_capture("b" * 64, "variant.SLDDRW", x_position=0.2)

    report = namespace["compare_results"](
        baseline,
        variant,
        baseline_api=baseline_api,
        variant_api=variant_api,
    )

    assert report == namespace["compare_results"](
        baseline,
        variant,
        baseline_api=baseline_api,
        variant_api=variant_api,
    )
    json.dumps(report, ensure_ascii=False, sort_keys=True)
    assert report["delta"]["source_changed"]
    assert report["delta"]["records"]["changed_count"] == 1
    assert report["delta"]["records"]["content_preserved_count"] == 2
    assert report["delta"]["record_hierarchy"]["removed_count"] == 1
    assert report["delta"]["record_hierarchy"]["added_count"] == 1
    assert report["delta"]["views"]["removed_count"] == 1
    assert report["delta"]["views"]["added_count"] == 1
    assert report["delta"]["candidate_streams"]["changed_count"] == 1
    assert report["solidworks_api"]["views"]["changed_count"] == 1
    assert report["solidworks_api"]["same_solidworks_environment"]
    assert not report["interpretation"]["binary_record_framing_verified"]
    assert not report["interpretation"]["api_to_native_record_mapping_verified"]


def test_differential_preserves_duplicate_record_multiplicity():
    namespace = _namespace()
    baseline = _result("a" * 64)
    duplicate = copy.deepcopy(baseline["structure"]["records"][2])
    duplicate["id"] = "view-duplicate"
    duplicate["source"]["decoded_offset"] = 30
    baseline["structure"]["records"].append(duplicate)
    variant = copy.deepcopy(baseline)
    variant["structure"]["source"]["sha256"] = "b" * 64
    variant["structure"]["records"].pop()

    report = namespace["compare_results"](baseline, variant)

    records = report["delta"]["records"]
    assert records["content_preserved_count"] == 3
    assert records["removed_count"] == 1
    assert records["removed"][0]["occurrences"] == 1


def test_differential_rejects_api_capture_for_different_source():
    namespace = _namespace()
    baseline = _result("a" * 64)
    variant = _result("b" * 64)
    error = namespace["DrawingDifferentialError"]

    with pytest.raises(error, match="baseline API capture SHA-256"):
        namespace["compare_results"](
            baseline,
            variant,
            baseline_api=_api_capture("c" * 64, "base.SLDDRW"),
            variant_api=_api_capture("b" * 64, "variant.SLDDRW"),
        )


def test_differential_rejects_corrupt_persistent_reference():
    namespace = _namespace()
    baseline = _result("a" * 64)
    variant = _result("b" * 64)
    baseline_api = _api_capture("a" * 64, "base.SLDDRW")
    baseline_api["drawing"]["sheets"][0]["views"][0]["persistent_reference"][
        "sha256"
    ] = "0" * 64
    error = namespace["DrawingDifferentialError"]

    with pytest.raises(error, match="persistent_reference.sha256"):
        namespace["compare_results"](
            baseline,
            variant,
            baseline_api=baseline_api,
            variant_api=_api_capture("b" * 64, "variant.SLDDRW"),
        )


def test_differential_cli_decodes_two_native_inputs(tmp_path, capsys):
    namespace = _namespace()
    api_tests = runpy.run_path(str(ROOT / "tests/test_api.py"))
    payload = api_tests["drawing_file"]()
    baseline = tmp_path / "baseline.SLDDRW"
    variant = tmp_path / "variant.SLDDRW"
    baseline.write_bytes(payload)
    variant.write_bytes(payload)

    exit_code = namespace["main"]([str(baseline), str(variant)])
    report = json.loads(capsys.readouterr().out)

    assert exit_code == 0
    assert not report["delta"]["source_changed"]
    assert report["delta"]["records"]["content_preserved_count"] == 9
    assert report["delta"]["candidate_streams"]["preserved_count"] == 3
