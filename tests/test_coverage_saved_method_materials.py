from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

import pytest

from scripts import coverage_band_companion as companion
from scripts import coverage_gate_calculation as gates
from scripts import coverage_study_acquire as acquire
from tests import test_coverage_stage_finalization as finalization_fixtures
from tests.test_coverage_gate_calculation import _fixture

ROOT = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study"
SAVED = ROOT / "saved-development-method-materials"
ORIGINAL = ROOT / "frozen-input-method-materials"


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def test_saved_method_preserves_candidate_quality_and_usage_limits():
    old = json.loads((ORIGINAL / "protocol.json").read_bytes())
    new = json.loads((SAVED / "protocol.json").read_bytes())
    for key in (
        "quality",
        "task_use",
        "provider_schedule",
        "acquisition",
        "capture",
        "consumer",
        "phase_limits",
        "variants",
        "primary_control",
        "candidate_source_sha256",
    ):
        assert new[key] == old[key], key
    assert (SAVED / "cohorts.json").read_bytes() == (ORIGINAL / "cohorts.json").read_bytes()
    assert _sha((SAVED / "protocol.json").read_bytes()) == companion.SAVED_PROTOCOL_SHA256
    assert _sha((SAVED / "band-companion-contract.json").read_bytes()) == companion.SAVED_CONTRACT_SHA256
    assert new["saved_development_origin"]["historical_grades_reused"] is False
    assert new["saved_development_origin"]["original_quality_credit"] is False
    assert acquire.pool_input_mode_for_stage(new, "development") == "saved-corpus/1"
    assert acquire.pool_input_mode_for_stage(new, "confirmation") == "frozen-pool/1"
    with pytest.raises(ValueError, match="stage kind required"):
        acquire.pool_input_mode_for_stage(new, None)


def test_saved_method_reaches_unchanged_quality_calculator_and_rejects_retuning():
    raw = (SAVED / "protocol.json").read_bytes()
    protocol = json.loads(raw)
    inputs = _fixture()
    inputs.update(
        protocol=protocol,
        protocol_sha256=_sha(raw),
        cohorts_sha256=companion.FROZEN_COHORTS_SHA256,
    )
    inputs["resource_evidence"].update(
        acquisition_timeout_seconds=protocol["acquisition"]["timeout_seconds"],
        acquisition_retries=protocol["acquisition"]["retries"],
        acquisition_pacing_seconds=protocol["acquisition"]["pacing_seconds"],
        arxiv_pacing_seconds=protocol["acquisition"]["arxiv_physical_pacing_seconds"],
    )
    report = gates.calculate_coverage_gate_report(**inputs)
    assert report.evaluation.status == "pass"
    changed = copy.deepcopy(inputs)
    changed["protocol"]["consumer"]["answerer_calls_maximum"] += 1
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**changed)
    changed = copy.deepcopy(inputs)
    changed["protocol_sha256"] = "0" * 64
    with pytest.raises(gates.GateCalculationError, match="frozen-execution-contract-mismatch"):
        gates.calculate_coverage_gate_report(**changed)


def test_saved_method_reaches_independent_finalizer(tmp_path):
    helper = finalization_fixtures.StageFinalizationTests()
    paths, plan = helper._fixture(
        tmp_path,
        elapsed=7.5,
        protocol_bytes_override=(SAVED / "protocol.json").read_bytes(),
        cohorts_bytes_override=(SAVED / "cohorts.json").read_bytes(),
    )
    result = helper._verify(paths, plan)
    assert result.status == "gates-pass-awaiting-independent-decision"
    assert json.loads(result.receipt_bytes)["product_authorized"] is False
