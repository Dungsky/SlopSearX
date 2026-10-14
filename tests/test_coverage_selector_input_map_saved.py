from __future__ import annotations

import hashlib
import json
from dataclasses import replace
from pathlib import Path

import pytest

from scripts import coverage_saved_corpus as saved
from scripts import coverage_selector_input_map as input_map
from tests import test_coverage_saved_corpus as saved_fixtures
from tests import test_coverage_selector_input_map as selector_fixtures


def _sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def _saved_kwargs(tmp_path: Path):
    kwargs = selector_fixtures._materials(tmp_path)
    origin, cases, navigation, _collection, _capture = saved_fixtures._origin_package(tmp_path)
    prepared = replace(
        kwargs["prepared"],
        registration_sha256="9" * 64,
        selector_input_map_required=True,
        selector_input_map_schema=input_map.REGISTERED_SCHEMA,
    )
    kwargs["prepared"] = prepared
    initial_registration_sha256 = "a" * 64
    band_companion_sha256 = "b" * 64
    current_cohorts_sha256 = origin.registration.cohorts_sha256
    binding = replace(
        saved_fixtures._current_binding(origin),
        stage_uuid=prepared.stage_uuid,
        source_revision=prepared.source_revision,
        source_closure_sha256=prepared.pins["qualified_source_closure"],
        protocol_sha256=prepared.pins["protocol"],
        cohorts_sha256=current_cohorts_sha256,
    )
    imported = saved.verify_and_import_saved_corpus(origin, binding, cases, navigation)
    preacquisition_input = {
        "schema": "coverage-preacquisition-input-manifest/1",
        "stage_uuid": prepared.stage_uuid,
        "protocol_sha256": prepared.pins["protocol"],
        "cohorts_sha256": current_cohorts_sha256,
        "task_ids": [row["task_id"] for row in cases] + [row["target_id"] for row in navigation],
        "band_companion_sha256": band_companion_sha256,
        "initial_registration_sha256": initial_registration_sha256,
    }
    import_plan = {
        "schema": "coverage-saved-import-plan/1",
        "stage": prepared.stage_kind,
        "stage_uuid": prepared.stage_uuid,
        "source_revision": prepared.source_revision,
        "source_closure_sha256": prepared.pins["qualified_source_closure"],
        "protocol_sha256": prepared.pins["protocol"],
        "cohorts_sha256": current_cohorts_sha256,
        "input_manifest_sha256": _sha(input_map._canonical(preacquisition_input)),
        "acquisition_plan_sha256": prepared.pins["acquisition_plan"],
        "research_cases": cases,
        "navigation_targets": navigation,
        "packet_stage_uuid": binding.packet_stage_uuid,
        "origin_package_sha256": origin.expected_package_sha256,
    }
    kwargs.update(
        {
            "snapshots_directory": imported.origin_pool_verified.snapshots_directory,
            "acquisition_snapshot_index_sha256": imported.origin_pool_verified.snapshot_index_sha256,
            "acquisition_manifest_bytes": saved._canonical(import_plan),
            "acquisition_manifest_sha256": _sha(saved._canonical(import_plan)),
            "verified_pool_source": imported.origin_pool_verified,
            "saved_corpus_import": imported,
            "initial_registration_sha256": initial_registration_sha256,
            "band_companion_sha256": band_companion_sha256,
        }
    )
    return kwargs, imported


def test_saved_selector_map_uses_origin_pool_and_binds_current_import(tmp_path):
    kwargs, imported = _saved_kwargs(tmp_path)
    plan = input_map._strict(kwargs["acquisition_manifest_bytes"])
    snapshot, verified_plan = input_map._saved_import_snapshot(
        prepared=kwargs["prepared"],
        acquisition_manifest=plan,
        acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
        acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
        snapshots_directory=kwargs["snapshots_directory"],
        verified_pool_source=kwargs["verified_pool_source"],
        saved_corpus_import=imported,
        initial_registration_sha256=kwargs["initial_registration_sha256"],
        band_companion_sha256=kwargs["band_companion_sha256"],
    )

    assert verified_plan["schema"] == "coverage-saved-import-plan/1"
    assert snapshot["index"]["stage_uuid"] == imported.origin_pool_verified.pool_index["stage_uuid"]
    assert snapshot["index"] is imported.origin_pool_verified.pool_index
    assert snapshot["operations"] is imported.origin_pool_verified.operations
    assert len(snapshot["operations"]) == 13


@pytest.mark.parametrize("tamper", ["receipt", "derived-manifest", "plan-stage", "index-pin"])
def test_saved_selector_map_rejects_rehashed_binding_drift(tmp_path, tamper):
    kwargs, imported = _saved_kwargs(tmp_path)
    plan = input_map._strict(kwargs["acquisition_manifest_bytes"])
    if tamper == "receipt":
        receipt = json.loads(imported.import_receipt_bytes)
        receipt["origin_package_sha256"] = "0" * 64
        forged = saved._canonical(receipt)
        kwargs["saved_corpus_import"] = replace(
            imported, import_receipt_bytes=forged, import_receipt_sha256=_sha(forged)
        )
    elif tamper == "derived-manifest":
        manifest = json.loads(imported.capture_manifest_bytes)
        manifest["source_revision"] = "0" * 40
        forged = saved._canonical(manifest)
        kwargs["saved_corpus_import"] = replace(
            imported, capture_manifest_bytes=forged, capture_manifest_sha256=_sha(forged)
        )
    elif tamper == "plan-stage":
        plan = json.loads(kwargs["acquisition_manifest_bytes"])
        plan["stage_uuid"] = "00000000-0000-0000-0000-000000000001"
        kwargs["acquisition_manifest_bytes"] = saved._canonical(plan)
        kwargs["acquisition_manifest_sha256"] = _sha(kwargs["acquisition_manifest_bytes"])
    else:
        kwargs["acquisition_snapshot_index_sha256"] = "0" * 64
    with pytest.raises(input_map.SelectorInputMapError):
        input_map._saved_import_snapshot(
            prepared=kwargs["prepared"],
            acquisition_manifest=input_map._strict(kwargs["acquisition_manifest_bytes"]),
            acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
            acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
            snapshots_directory=kwargs["snapshots_directory"],
            verified_pool_source=kwargs["verified_pool_source"],
            saved_corpus_import=kwargs["saved_corpus_import"],
            initial_registration_sha256=kwargs.get("initial_registration_sha256"),
            band_companion_sha256=kwargs.get("band_companion_sha256"),
        )


@pytest.mark.parametrize("missing", ["initial_registration_sha256", "band_companion_sha256"])
def test_registered_saved_map_requires_external_initial_and_companion_pins(tmp_path, missing):
    kwargs, imported = _saved_kwargs(tmp_path)
    kwargs.pop(missing)
    with pytest.raises(input_map.SelectorInputMapError, match="saved-import-registration-pins-required"):
        input_map._saved_import_snapshot(
            prepared=kwargs["prepared"],
            acquisition_manifest=input_map._strict(kwargs["acquisition_manifest_bytes"]),
            acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
            acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
            snapshots_directory=kwargs["snapshots_directory"],
            verified_pool_source=kwargs["verified_pool_source"],
            saved_corpus_import=imported,
            initial_registration_sha256=kwargs.get("initial_registration_sha256"),
            band_companion_sha256=kwargs.get("band_companion_sha256"),
        )


@pytest.mark.parametrize("wrong", ["initial_registration_sha256", "band_companion_sha256"])
def test_registered_saved_map_rejects_wrong_external_registration_pins(tmp_path, wrong):
    kwargs, imported = _saved_kwargs(tmp_path)
    kwargs[wrong] = "c" * 64
    with pytest.raises(input_map.SelectorInputMapError, match="saved-import-plan-input-manifest-invalid"):
        input_map._saved_import_snapshot(
            prepared=kwargs["prepared"],
            acquisition_manifest=input_map._strict(kwargs["acquisition_manifest_bytes"]),
            acquisition_manifest_sha256=kwargs["acquisition_manifest_sha256"],
            acquisition_snapshot_index_sha256=kwargs["acquisition_snapshot_index_sha256"],
            snapshots_directory=kwargs["snapshots_directory"],
            verified_pool_source=kwargs["verified_pool_source"],
            saved_corpus_import=imported,
            initial_registration_sha256=kwargs["initial_registration_sha256"],
            band_companion_sha256=kwargs["band_companion_sha256"],
        )
