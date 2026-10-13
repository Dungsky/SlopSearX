"""Companion qualification for the two frozen research-size bands.

This is an additive, opt-in registered contract. The original mixed-workload
calculator and its protocol thresholds remain unchanged. The companion uses
the same closed grades and the same ranking/task-use implementations, split
only by the predeclared cohort membership.
"""

from __future__ import annotations

import hashlib
import json
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from types import SimpleNamespace

from scripts.coverage_gate_calculation import (
    FROZEN_QUALITY_TASK_USE_SHA256,
    Endpoint,
    _ranking_endpoint,
    _task_use,
)

SCHEMA = "coverage-first-band-companion/1"
PROTOCOL_SHA256 = "567e337d41268f380348abc70ab1bf4a0249852f237797fa361295f1e34afb06"
COHORTS_SHA256 = "36d4cbff9c93555d35d7269525616f009a21727c8d1141a8a2a61934791038f0"
GROUPS = {
    "research_le_40": {
        "development": ["D-R01", "D-R02", "D-R03", "D-R04"],
        "confirmation": ["C-R01", "C-R02", "C-R03", "C-R04"],
    },
    "research_41_80": {
        "development": ["D-R05", "D-R06", "D-R07", "D-R08"],
        "confirmation": ["C-R05", "C-R06", "C-R07", "C-R08"],
    },
}


class BandCompanionError(ValueError):
    """The registered companion declaration or its closed inputs are invalid."""


@dataclass(frozen=True)
class BandCompanionResult:
    status: str
    groups: Mapping[str, object]
    gates: Mapping[str, bool | None]


def validate_contract(contract: object, *, stage_kind: str, protocol_sha256: str, cohorts_sha256: str) -> dict:
    if type(stage_kind) is not str or stage_kind not in {"development", "confirmation"}:
        raise BandCompanionError("companion-stage-kind-invalid")
    if type(contract) is not dict or set(contract) != {
        "schema",
        "stage_kind",
        "protocol_sha256",
        "cohorts_sha256",
        "groups",
        "claim_scope",
    }:
        raise BandCompanionError("companion-contract-shape")
    if contract["schema"] != SCHEMA or contract["stage_kind"] != stage_kind:
        raise BandCompanionError("companion-contract-identity")
    if contract["protocol_sha256"] != PROTOCOL_SHA256 or protocol_sha256 != PROTOCOL_SHA256:
        raise BandCompanionError("companion-protocol-pin-mismatch")
    if contract["cohorts_sha256"] != COHORTS_SHA256 or cohorts_sha256 != COHORTS_SHA256:
        raise BandCompanionError("companion-cohort-pin-mismatch")
    if contract["groups"] != GROUPS:
        raise BandCompanionError("companion-group-inventory-mismatch")
    if contract["claim_scope"] != "fixed-frozen-cases-only; no population or causal-depth claim":
        raise BandCompanionError("companion-claim-scope-invalid")
    return contract


def _validate_frozen_recipes(quality: Mapping[str, object], task_use: Mapping[str, object]) -> None:
    try:
        raw = json.dumps(
            {"quality": quality, "task_use": task_use},
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
            allow_nan=False,
        ).encode("utf-8")
    except (TypeError, ValueError, UnicodeEncodeError) as exc:
        raise BandCompanionError("companion-gate-recipe-invalid") from exc
    if hashlib.sha256(raw).hexdigest() != FROZEN_QUALITY_TASK_USE_SHA256:
        raise BandCompanionError("companion-frozen-gate-recipe-mismatch")


def calculate(
    *,
    contract: Mapping[str, object],
    stage_kind: str,
    protocol_sha256: str,
    cohorts_sha256: str,
    quality: Mapping[str, object],
    task_use: Mapping[str, object],
    protocol: Mapping[str, object],
    task_inputs: Sequence[Mapping[str, object]],
    orders: Mapping[str, object],
    records_by_task: Mapping[str, object],
    answer_scores: object,
    prepared_assessments: object,
) -> BandCompanionResult:
    _validate_frozen_recipes(quality, task_use)
    validate_contract(
        dict(contract), stage_kind=stage_kind, protocol_sha256=protocol_sha256, cohorts_sha256=cohorts_sha256
    )
    groups: dict[str, object] = {}
    gates: dict[str, bool | None] = {}
    observed = {row.get("task_id"): row for row in task_inputs if type(row) is dict}
    catalog = getattr(prepared_assessments, "task_catalogs", None)
    for band, stage_ids in GROUPS.items():
        task_ids = stage_ids[stage_kind]
        rows = [observed.get(task_id) for task_id in task_ids]
        valid = (
            len(observed) == len(task_inputs) == 8
            and all(type(row) is dict and row.get("band") == band for row in rows)
            and all(row.get("intent") in {"evidence_seeking", "source_discovery"} for row in rows)
            and sum(row.get("intent") == "evidence_seeking" for row in rows) == 2
            and sum(row.get("intent") == "source_discovery" for row in rows) == 2
        )
        if not valid:
            for assessor in quality["independent_references"]:
                gates[f"band_{band}_{assessor}_ranking"] = None
            for assessor in task_use["independent_assessors"]:
                gates[f"band_{band}_{assessor}_task_use"] = None
            groups[band] = {"status": "inconclusive", "reason": "registered-band-membership-incomplete"}
            continue
        ranking = {
            assessor: _ranking_endpoint(assessor, rows, orders, records_by_task, quality)
            for assessor in quality["independent_references"]
        }
        band_answers = (
            {
                task_id: answer_scores[task_id]
                for task_id in task_ids
                if type(answer_scores) is dict and task_id in answer_scores
            }
            if type(answer_scores) is dict
            else None
        )
        band_catalogs = (
            {task_id: catalog[task_id] for task_id in task_ids if type(catalog) is dict and task_id in catalog}
            if type(catalog) is dict
            else None
        )
        band_prepared = SimpleNamespace(task_catalogs=band_catalogs)
        task_results: dict[str, Endpoint] = {}
        if type(band_answers) is dict:
            for assessor in task_use["independent_assessors"]:
                filtered = {
                    task_id: {
                        arm: ({assessor: graders[assessor]} if type(graders) is dict and assessor in graders else {})
                        for arm, graders in arms.items()
                    }
                    for task_id, arms in band_answers.items()
                    if type(arms) is dict
                }
                task_results[assessor] = _task_use(
                    filtered,
                    band_prepared,
                    task_ids,
                    task_use["additional_completed_tasks_each_minimum"],
                    protocol,
                    (assessor,),
                )
        else:
            task_results = {
                assessor: Endpoint(None, {"reason": "answer-output-unavailable"})
                for assessor in task_use["independent_assessors"]
            }
        for assessor, result in ranking.items():
            gates[f"band_{band}_{assessor}_ranking"] = result.passed
        for assessor, result in task_results.items():
            gates[f"band_{band}_{assessor}_task_use"] = result.passed
        values = [*ranking.values(), *task_results.values()]
        group_status = (
            "inconclusive"
            if any(item.passed is None for item in values)
            else "fail"
            if any(item.passed is False for item in values)
            else "pass"
        )
        groups[band] = {
            "status": group_status,
            "task_ids": task_ids,
            "ranking": {key: value.metrics for key, value in ranking.items()},
            "task_use": {key: value.metrics for key, value in task_results.items()},
        }
    status = (
        "inconclusive"
        if any(value is None for value in gates.values())
        else "fail"
        if any(value is False for value in gates.values())
        else "pass"
    )
    return BandCompanionResult(status=status, groups=groups, gates=gates)
