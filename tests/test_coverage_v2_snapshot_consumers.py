"""Profile binding across the complete v2 acquisition snapshot consumers."""

from __future__ import annotations

import hashlib
import json
import uuid
from dataclasses import replace
from pathlib import Path
from urllib.parse import parse_qs

import httpx
import pytest

from scripts import coverage_late_registration as late
from scripts import coverage_live_acquire as live
from scripts import coverage_pipeline_inputs as pipeline
from scripts import coverage_selector_input_map as selector_map
from scripts import coverage_stage_orchestration as orchestration
from scripts import coverage_study_acquire as acquisition
from scripts import coverage_study_core as core
from tests.test_coverage_live_acquire import FakePacer, FakePermitVerifier, OneShot
from tests.test_coverage_stage_orchestration import (
    _source_closure_bytes,
    _source_materials,
    canonical,
    digest,
    plan_fixture,
)


def _v2_plan() -> orchestration.StagePlan:
    plan = plan_fixture(selector_map_registered=True)
    protocol_bytes = (
        Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/protocol-acquisition-v2-draft.json"
    ).read_bytes()
    manifest = json.loads(plan.acquisition_manifest_bytes)
    manifest.update(
        {
            "schema": acquisition.CONTINUE_ACQUISITION_PROFILE.manifest_schema,
            "execution_profile": acquisition.CONTINUE_ACQUISITION_PROFILE.identifier,
            "protocol_sha256": digest(protocol_bytes),
        }
    )
    manifest["input_manifest_sha256"] = digest(
        canonical(
            {
                "schema": "coverage-preacquisition-input-manifest/1",
                "stage_uuid": plan.stage_uuid,
                "protocol_sha256": digest(protocol_bytes),
                "cohorts_sha256": digest(plan.cohorts_bytes),
                "task_ids": [row["task_id"] for row in plan.research_cases]
                + [row["target_id"] for row in plan.navigation_targets],
            }
        )
    )
    manifest_bytes = live._canonical(manifest)
    return replace(
        plan,
        protocol_bytes=protocol_bytes,
        acquisition_manifest_bytes=manifest_bytes,
        expected_acquisition_manifest_sha256=digest(manifest_bytes),
    )


def _mock_response_handler(plan: orchestration.StagePlan, requests: list[httpx.Request]):
    nav_query_to_target = {row["search_query"]: row["target_url"] for row in plan.navigation_targets}

    def handle(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        query = parse_qs(request.url.query.decode())
        term = query.get("q", query.get("search", [""]))[0]
        key = hashlib.sha1(term.encode()).hexdigest()[:8]
        if request.url.host == "api.github.com":
            target = nav_query_to_target.get(term)
            rows = []
            if target is not None:
                owner, repo = target.rstrip("/").rsplit("/", 2)[-2:]
                rows.append(
                    {
                        "full_name": f"{owner}/{repo}",
                        "html_url": target,
                        "description": "Synthetic target repository.",
                        "stargazers_count": 100,
                        "language": "Python",
                    }
                )
            rows.extend(
                {
                    "full_name": f"fixture-{key}/github-{index}",
                    "html_url": f"https://github.com/fixture-{key}/github-{index}",
                    "description": f"Synthetic GitHub evidence {index} for {term}.",
                    "stargazers_count": 50 - index,
                    "language": "Python",
                }
                for index in range(20 - len(rows))
            )
            return httpx.Response(200, json={"total_count": len(rows), "items": rows}, request=request)

        if request.url.host == "api.openalex.org":
            rows = [
                {
                    "id": f"https://openalex.org/W{key}{index:04d}",
                    "doi": f"https://doi.org/10.5555/{key}-{index}",
                    "title": f"Synthetic OpenAlex evidence {index} for {term}.",
                    "publication_date": "2025-01-01",
                    "relevance_score": 9.5 - index / 100,
                    "abstract_inverted_index": None,
                }
                for index in range(20)
            ]
            return httpx.Response(200, json={"results": rows}, request=request)

        if request.url.host == "export.arxiv.org":
            entries = "".join(
                "<entry>"
                f"<id>https://arxiv.org/abs/2601.{int(key, 16) * 100 + index:05d}</id>"
                f"<title>Synthetic arXiv evidence {index} for {term}</title>"
                f"<summary>Bounded synthetic evidence for {term}, result {index}.</summary>"
                "<published>2025-01-01T00:00:00Z</published></entry>"
                for index in range(20)
            )
            body = f'<feed xmlns="http://www.w3.org/2005/Atom">{entries}</feed>'
            return httpx.Response(
                200, content=body.encode(), headers={"content-type": "application/atom+xml"}, request=request
            )

        if request.url.host == "en.wikipedia.org":
            if query.get("action") == ["opensearch"]:
                titles = [f"Fixture {key} Evidence {index}" for index in range(20)]
                return httpx.Response(
                    200,
                    json=[term, titles, ["https://en.wikipedia.org/" for _ in titles], ["fixture" for _ in titles]],
                    request=request,
                )
            titles = query.get("titles", [""])[0].split("|")
            pages = {
                str(index + 1): {
                    "pageid": index + 1,
                    "title": title,
                    "extract": f"Synthetic Wikipedia evidence for {term}, result {index}.",
                }
                for index, title in enumerate(titles)
            }
            return httpx.Response(200, json={"query": {"pages": pages}}, request=request)

        raise AssertionError(f"Unexpected mocked host: {request.url.host}")

    return handle


def _late_core_materials(plan: orchestration.StagePlan, task_input: bytes, capture_manifest: bytes) -> dict[str, bytes]:
    source_materials = _source_materials(registered=True)
    task_ids = [row["task_id"] for row in plan.research_cases] + [row["target_id"] for row in plan.navigation_targets]
    return {
        "protocol": plan.protocol_bytes,
        "qualified_source_closure": _source_closure_bytes(plan.source_revision, registered=True),
        **source_materials,
        "task_input_manifest": task_input,
        "source_capture_manifest": capture_manifest,
        "reference_manifest": core._canonical(
            {"schema": "coverage-blind-reference-manifest/1", "stage_uuid": plan.stage_uuid}
        ),
        "answer_assessment_plan": core._canonical(
            {"schema": "coverage-answer-assessment-plan/1", "stage_uuid": plan.stage_uuid}
        ),
        "acquisition_plan": plan.acquisition_plan_bytes,
        "capture_plan": core._canonical(
            {
                "schema": core.CAPTURE_SCHEMA,
                "stage_uuid": plan.stage_uuid,
                "tasks": [{"task_id": task_id, "scraper_calls": 1} for task_id in task_ids],
                "health_calls": 1,
            }
        ),
    }


def _prepared_from_materials(plan: orchestration.StagePlan, materials: dict[str, bytes]) -> core.PreparedStage:
    pins = {key: digest(value) for key, value in materials.items()}
    task_ids = tuple(row["task_id"] for row in plan.research_cases) + tuple(
        row["target_id"] for row in plan.navigation_targets
    )
    skips = (None,) * 5
    registration = core._canonical(
        {
            "schema": core.REGISTRATION_SCHEMA,
            "stage_uuid": plan.stage_uuid,
            "stage_kind": plan.stage_kind,
            "source_revision": plan.source_revision,
            "pins": pins,
            "operation_ids": [row.operation_id for row in core.selector_inventory(skips)],
            "navigation_skip_reasons": list(skips),
            "task_ids": list(task_ids),
        }
    )
    return core.preflight_stage(
        registration_bytes=registration,
        expected_registration_sha256=digest(registration),
        expected_stage_uuid=plan.stage_uuid,
        forbidden_stage_uuids=(str(uuid.UUID(int=8000)),),
        expected_source_revision=plan.source_revision,
        observed_source_revision=plan.source_revision,
        materials=materials,
        expected_pins=pins,
        acquisition_plan_bytes=plan.acquisition_plan_bytes,
        capture_plan_bytes=materials["capture_plan"],
        task_ids=task_ids,
        navigation_skip_reasons=skips,
    )


@pytest.mark.asyncio
async def test_complete_v2_snapshots_flow_through_pipeline_late_materials_and_selector_map(tmp_path):
    plan = _v2_plan()
    requests: list[httpx.Request] = []
    receipt = b"synthetic admitted permit fixture"
    acquired = await live.acquire_live_coverage_stage(
        stage_manifest=json.loads(plan.acquisition_manifest_bytes),
        acquisition_plan_bytes=plan.acquisition_plan_bytes,
        permit_receipt_bytes=receipt,
        expected_permit_receipt_sha256=digest(receipt),
        permit_verifier=FakePermitVerifier(),
        one_shot_lease=OneShot(),
        receipt_directory=tmp_path / "v2-acquisition",
        test_transport=httpx.MockTransport(_mock_response_handler(plan, requests)),
        pacer=FakePacer(),
    )
    assert acquired.stage.status == "complete"
    assert len(acquired.stage.operations) == 13
    assert all(row.status == "complete" for row in acquired.stage.operations)
    assert acquired.stage.physical_request_count == len(requests)
    assert len(requests) <= acquisition.MAX_PHYSICAL_REQUESTS_PER_STAGE
    assert [row.pool_count for row in acquired.stage.operations[:4]] == [40] * 4
    assert [row.pool_count for row in acquired.stage.operations[4:8]] == [80] * 4
    verified = live.verify_pool_snapshot_index(
        acquired.receipt_directory,
        expected_index_sha256=acquired.pool_snapshot_index_sha256,
        expected_stage_manifest_sha256=plan.expected_acquisition_manifest_sha256,
        expected_stage_uuid=plan.stage_uuid,
        expected_source_revision=plan.source_revision,
        expected_execution_profile=acquisition.CONTINUE_ACQUISITION_PROFILE.identifier,
    )
    assert verified["index"]["execution_profile"] == acquisition.CONTINUE_ACQUISITION_PROFILE.identifier

    pipeline_inputs = pipeline.build_pipeline_inputs(
        list(plan.research_cases),
        snapshots_directory=acquired.receipt_directory,
        expected_snapshot_index_sha256=acquired.pool_snapshot_index_sha256,
        expected_stage_manifest_sha256=plan.expected_acquisition_manifest_sha256,
        stage_uuid=plan.stage_uuid,
        source_revision=plan.source_revision,
        protocol_sha256=digest(plan.protocol_bytes),
        cohorts_sha256=digest(plan.cohorts_bytes),
        candidate_identity_sha256=digest(plan.candidate_identity_bytes),
        candidate_endpoint_sha256=plan.candidate_endpoint_sha256,
        expected_execution_profile=acquisition.CONTINUE_ACQUISITION_PROFILE.identifier,
    )
    assert len(pipeline_inputs["tasks"]) == 8
    evidence = orchestration.AcquisitionEvidence(
        result=acquired.stage,
        snapshots_directory=acquired.receipt_directory,
        snapshot_index_sha256=acquired.pool_snapshot_index_sha256,
        acquisition_manifest_sha256=acquired.stage_manifest_sha256,
        receipt_sha256=digest(b"synthetic durable source receipt"),
        source_bound_receipt_status="synthetic-test-only",
    )
    static_materials = {
        key: value
        for key, value in _late_core_materials(
            plan,
            b"{}",  # Replaced by the post-acquisition builder below.
            pipeline_inputs["capture_manifest_bytes"],
        ).items()
        if key not in {"task_input_manifest", "source_capture_manifest"}
    }
    late_materials = late.build_postacquisition_materials(
        plan=plan,
        acquisition_evidence=evidence,
        pipeline_inputs=pipeline_inputs,
        static_materials=static_materials,
    )
    prepared = _prepared_from_materials(plan, late_materials)
    nav_targets = [
        {"target_id": row["target_id"], "search_query": row["search_query"]} for row in plan.navigation_targets
    ]
    fixture_path = (
        Path(__file__).parents[1]
        / "docs/experiments/evidence/coverage-first-study/draft-v2/selector-readiness-inputs.json"
    )
    frozen_v1 = (
        Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
    ).read_bytes()
    source_bytes = Path(selector_map.__file__).parents[1] / "slopsearx/service.py"
    operation_materials: dict[str, dict[str, object]] = {}
    selector_kwargs = {
        "prepared": prepared,
        "task_input_manifest_bytes": late_materials["task_input_manifest"],
        "pipeline_tasks": pipeline_inputs["tasks"],
        "navigation_targets": nav_targets,
        "snapshots_directory": acquired.receipt_directory,
        "acquisition_snapshot_index_sha256": acquired.pool_snapshot_index_sha256,
        "acquisition_manifest_bytes": plan.acquisition_manifest_bytes,
        "acquisition_manifest_sha256": plan.expected_acquisition_manifest_sha256,
        "neutral_fixture_bytes": fixture_path.read_bytes(),
        "frozen_v1_source_bytes": frozen_v1,
        "current_service_source_bytes": source_bytes.read_bytes(),
        "expected_builder_source_sha256": digest(Path(selector_map.__file__).read_bytes()),
        "operation_materials": operation_materials,
    }
    selector_map_bytes = selector_map.build_selector_input_map(**selector_kwargs)
    selector_doc = json.loads(selector_map_bytes)
    assert selector_doc["status"] == "complete-awaiting-external-admission"
    assert len(selector_doc["operations"]) == len(core.selector_inventory((None,) * 5))
    assert len(operation_materials) == len(selector_doc["operations"])

    # The old default is deliberately not inferred from a v2 snapshot index.
    with pytest.raises(live.LiveAcquisitionError, match="pool-snapshot-index-incomplete"):
        live.verify_pool_snapshot_index(
            acquired.receipt_directory,
            expected_index_sha256=acquired.pool_snapshot_index_sha256,
            expected_stage_manifest_sha256=plan.expected_acquisition_manifest_sha256,
            expected_stage_uuid=plan.stage_uuid,
            expected_source_revision=plan.source_revision,
        )

    missing_profile = json.loads(plan.acquisition_manifest_bytes)
    del missing_profile["execution_profile"]
    missing_profile["schema"] = acquisition.LEGACY_ACQUISITION_PROFILE.manifest_schema
    missing_bytes = live._canonical(missing_profile)
    with pytest.raises(late.LateRegistrationError, match="postacquisition-profile-binding-invalid"):
        late.build_postacquisition_materials(
            plan=replace(
                plan,
                acquisition_manifest_bytes=missing_bytes,
                expected_acquisition_manifest_sha256=digest(missing_bytes),
            ),
            acquisition_evidence=evidence,
            pipeline_inputs=pipeline_inputs,
            static_materials=static_materials,
        )

    wrong_profile = json.loads(plan.acquisition_manifest_bytes)
    wrong_profile["execution_profile"] = acquisition.LEGACY_ACQUISITION_PROFILE.identifier
    wrong_bytes = live._canonical(wrong_profile)
    with pytest.raises(late.LateRegistrationError, match="postacquisition-profile-binding-invalid"):
        late.build_postacquisition_materials(
            plan=replace(
                plan,
                acquisition_manifest_bytes=wrong_bytes,
                expected_acquisition_manifest_sha256=digest(wrong_bytes),
            ),
            acquisition_evidence=evidence,
            pipeline_inputs=pipeline_inputs,
            static_materials=static_materials,
        )

    wrong_selector_kwargs = {
        **selector_kwargs,
        "acquisition_manifest_bytes": missing_bytes,
        "acquisition_manifest_sha256": digest(missing_bytes),
    }
    with pytest.raises(selector_map.SelectorInputMapError, match="acquisition-snapshot-verification-failed"):
        selector_map.build_selector_input_map(**wrong_selector_kwargs)

    wrong_selector_manifest = json.loads(plan.acquisition_manifest_bytes)
    wrong_selector_manifest["execution_profile"] = acquisition.LEGACY_ACQUISITION_PROFILE.identifier
    wrong_selector_bytes = live._canonical(wrong_selector_manifest)
    wrong_selector_kwargs = {
        **selector_kwargs,
        "acquisition_manifest_bytes": wrong_selector_bytes,
        "acquisition_manifest_sha256": digest(wrong_selector_bytes),
    }
    with pytest.raises(selector_map.SelectorInputMapError, match="acquisition-manifest-profile-invalid"):
        selector_map.build_selector_input_map(**wrong_selector_kwargs)
