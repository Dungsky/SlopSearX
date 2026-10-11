from __future__ import annotations

import asyncio
import hashlib
import json
import os
import stat
import subprocess
import tempfile
import unittest
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

from scripts import coverage_answer_execution as answer
from scripts import coverage_operator_runner as runner
from tests.test_coverage_answer_execution import _stage_inputs, _tasks
from tests.test_coverage_stage_orchestration import plan_fixture


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


class OperatorRunnerConfigTests(unittest.TestCase):
    def _config(self, root: Path, protocol: bytes) -> tuple[Path, str]:
        path_values = {key: root / f"{key}.json" for key in runner._PATH_KEYS}
        for key, path in path_values.items():
            path.write_bytes(protocol if key == "protocol" else b"{}")
        private_paths = {key: root / f"{key}.secret" for key in runner._PRIVATE_PATH_KEYS}
        for path in private_paths.values():
            path.write_bytes(b"synthetic-test-only\n")
            path.chmod(0o600)
        directories = {key: str(root / key) for key in runner._DIRECTORY_KEYS}
        value = {
            "schema": runner.CONFIG_SCHEMA,
            "stage_uuid": str(uuid.UUID(int=501)),
            "packet_stage_uuid": str(uuid.UUID(int=502)),
            "stage_kind": "development",
            "source_revision": "a" * 40,
            "candidate_base_url": "https://candidate.example",
            "answer_endpoint": "https://model.example/v1/chat/completions",
            "initial_registration_sha256": "1" * 64,
            "forbidden_stage_uuids": [str(uuid.UUID(int=100))],
            "paths": {key: str(path) for key, path in path_values.items()},
            "private_paths": {key: str(path) for key, path in private_paths.items()},
            "directories": directories,
        }
        config_path = root / "operator-config.json"
        raw = canonical(value)
        config_path.write_bytes(raw)
        config_path.chmod(0o600)
        return config_path, digest(raw)

    def _registered_config(self, root: Path) -> tuple[Path, str, dict[str, str]]:
        """Make a synthetic, internally pinned config that reaches root prep."""
        bundle = Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/registered-2026-10-11"
        stage_dir = bundle / "development"
        external_paths = {
            "cohorts": bundle / "cohorts.json",
            "protocol": bundle / "protocol.json",
            "qualified_source_closure": bundle / "current-qualified-source-closure.json",
            "coverage_source": Path(__file__).parents[1] / "scripts/intent_ranking_coverage.py",
            "production_rerank_source": (
                Path(__file__).parents[1] / "docs/experiments/evidence/coverage-first-study/w0-rerank-v1.py.txt"
            ),
            "dependency_lock": Path(__file__).parents[1] / "uv.lock",
            "reference_manifest": stage_dir / "reference-manifest-shell.json",
            "answer_assessment_plan": stage_dir / "answer-assessment-plan-shell.json",
            "capture_plan": stage_dir / "capture-plan.json",
            "candidate_identity": bundle / "candidate-runtime-identity.json",
        }
        config_path, _ = self._config(root, external_paths["protocol"].read_bytes())
        config = json.loads(config_path.read_bytes())
        config["source_revision"] = "a" * 40
        config["candidate_base_url"] = "https://candidate.example"
        stage_uuid = config["stage_uuid"]
        path_values = {key: Path(value) for key, value in config["paths"].items()}
        material_bytes = {key: path.read_bytes() for key, path in external_paths.items()}
        material_bytes["qualified_source_closure"] = runner._canonical(
            {
                "schema": "coverage-study-qualified-source-closure/1",
                "source_revision": config["source_revision"],
                "material_pins": {
                    "coverage_source": digest(material_bytes["coverage_source"]),
                    "production_rerank_source": digest(material_bytes["production_rerank_source"]),
                    "dependency_lock": digest(material_bytes["dependency_lock"]),
                },
            }
        )
        for key, raw in material_bytes.items():
            path_values[key].write_bytes(raw)

        cohorts = runner.core._strict_json(material_bytes["cohorts"], "test-cohorts")
        stage_row = runner._stage_cohort(cohorts, config["stage_kind"])
        acquisition_plan = runner._build_acquisition_plan(
            stage_uuid, stage_row["research_cases"], stage_row["navigation_targets"]
        )
        static_materials = {
            "protocol": material_bytes["protocol"],
            "qualified_source_closure": material_bytes["qualified_source_closure"],
            "coverage_source": material_bytes["coverage_source"],
            "production_rerank_source": material_bytes["production_rerank_source"],
            "dependency_lock": material_bytes["dependency_lock"],
            "reference_manifest": material_bytes["reference_manifest"],
            "answer_assessment_plan": material_bytes["answer_assessment_plan"],
            "acquisition_plan": acquisition_plan,
            "capture_plan": material_bytes["capture_plan"],
        }
        registration = runner._canonical(
            {
                "stage_uuid": stage_uuid,
                "pins": {
                    **{key: digest(raw) for key, raw in static_materials.items()},
                    "task_input_manifest": "b" * 64,
                    "source_capture_manifest": "c" * 64,
                },
            }
        )
        path_values["initial_registration"].write_bytes(registration)
        config["initial_registration_sha256"] = digest(registration)
        config["paths"] = {key: str(path) for key, path in path_values.items()}
        raw_config = canonical(config)
        config_path.write_bytes(raw_config)
        config_path.chmod(0o600)
        return config_path, digest(raw_config), config["directories"]

    def test_unregistered_or_draft_protocol_fails_before_clock_or_handoff_side_effects(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            protocol = canonical({"selector_input_map_schema": "coverage-selector-input-map/2-draft"})
            config, pin = self._config(root, protocol)
            with (
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
                mock.patch.object(
                    runner, "_prepare_private_directories", side_effect=AssertionError("no root preparation")
                ),
                mock.patch.object(
                    runner.operator_handoff, "OperatorReceiptHandoff", side_effect=AssertionError("no handoff")
                ),
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "registered-source-materials-required"):
                    runner.OperatorStageRunner.from_file(config, pin)
            self.assertFalse((root / "operator_handoff").exists())
            configured_directories = json.loads(config.read_bytes())["directories"].values()
            self.assertTrue(all(not Path(path).exists() for path in configured_directories))

    def test_registered_config_creates_all_private_roots_before_clock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, pin, directories = self._registered_config(root)

            def clock_after_root_preparation():
                for value in directories.values():
                    info = Path(value).lstat()
                    self.assertTrue(stat.S_ISDIR(info.st_mode))
                    self.assertFalse(stat.S_ISLNK(info.st_mode))
                    self.assertEqual(stat.S_IMODE(info.st_mode), 0o700)
                    self.assertEqual(info.st_uid, os.geteuid())
                return 123.0

            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner.core, "_verify_qualified_source_closure"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                mock.patch.object(runner.orchestration, "_validate_plan"),
                mock.patch.object(runner.time, "monotonic", side_effect=clock_after_root_preparation),
            ):
                instance = runner.OperatorStageRunner.from_file(config, pin)

            self.assertEqual(instance.started_monotonic, 123.0)
            self.assertEqual(set(directories), runner._DIRECTORY_KEYS)

    def test_registered_pin_mismatch_fails_before_clock_or_root_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin, directories = self._registered_config(root)
            config_value = json.loads(config_path.read_bytes())
            registration_path = Path(config_value["paths"]["initial_registration"])
            registration = json.loads(registration_path.read_bytes())
            registration["pins"]["protocol"] = "d" * 64
            registration_raw = canonical(registration)
            registration_path.write_bytes(registration_raw)
            config_value["initial_registration_sha256"] = digest(registration_raw)
            config_raw = canonical(config_value)
            config_path.write_bytes(config_raw)
            config_path.chmod(0o600)

            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner.core, "_verify_qualified_source_closure"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
            ):
                with self.assertRaisesRegex(
                    runner.OperatorRunnerError, "initial-registration-static-pin-mismatch:protocol"
                ):
                    runner.OperatorStageRunner.from_file(config_path, digest(config_raw))

            self.assertTrue(all(not Path(value).exists() for value in directories.values()))

    def test_unsafe_existing_private_root_fails_before_clock_or_other_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, pin, directories = self._registered_config(root)
            unsafe = Path(directories["stage_inventory"])
            unsafe.mkdir(mode=0o755)
            unsafe.chmod(0o755)

            with (
                mock.patch.object(runner, "_verify_checkout_sources"),
                mock.patch.object(runner, "_require_source_bootstrap"),
                mock.patch.object(runner.core, "_verify_qualified_source_closure"),
                mock.patch.object(runner.runtime_environment, "verify_active_environment", return_value=b"{}"),
                mock.patch.object(runner, "_prepare_private_directories", wraps=runner._prepare_private_directories),
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "configured-private-root-unsafe"):
                    runner.OperatorStageRunner.from_file(config, pin)

            self.assertEqual(stat.S_IMODE(unsafe.stat().st_mode), 0o755)
            self.assertTrue(
                all(Path(value).exists() is (key == "stage_inventory") for key, value in directories.items())
            )

    def test_config_expected_digest_is_external_and_secret_paths_are_private(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config, pin = self._config(root, b"{}")
            with self.assertRaisesRegex(runner.OperatorRunnerError, "config-external-pin-mismatch"):
                runner.OperatorStageRunner.from_file(config, "0" * 64)
            secret = root / "candidate_operator_token.secret"
            secret.chmod(0o644)
            with self.assertRaisesRegex(runner.OperatorRunnerError, "configured-secret-file-not-private"):
                runner._secret(str(secret))

    def test_optional_ca_bundle_is_an_explicit_private_config_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin = self._config(root, b"{}")
            config = json.loads(config_path.read_bytes())
            ca_path = root / "capture-ca.pem"
            ca_path.write_text("-----BEGIN CERTIFICATE-----\nsynthetic\n", encoding="ascii")
            ca_path.chmod(0o600)
            config["private_paths"]["capture_ca_bundle"] = str(ca_path)
            raw = canonical(config)
            config_path.write_bytes(raw)
            config_path.chmod(0o600)
            loaded = runner._load_config(config_path, digest(raw))
            self.assertEqual(loaded["private_paths"]["capture_ca_bundle"], str(ca_path))

    def test_fresh_runtime_handoff_requires_native_host_transcripts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            secret = root / "selector.key"
            secret.write_bytes(b"synthetic-test-only\n")
            secret.chmod(0o600)
            instance = object.__new__(runner.OperatorStageRunner)
            instance.directories = {
                key: str(root / key)
                for key in {
                    "grader_handoff",
                    "lease_root",
                    "selector_archive",
                    "selector_results",
                }
            }
            instance.private_paths = {"selector_api_key": str(secret)}

            bindings = instance.runtime_bindings()

            self.assertTrue(bindings.native_handoff.require_native_host_transcripts)

    def test_checkout_audit_rejects_revision_dirty_tree_and_poisoned_material(self):
        revision = subprocess.run(
            ["git", "rev-parse", "--verify", "HEAD^{commit}"],
            cwd=runner._REPOSITORY_ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
        paths = {key: str(path) for key, path in runner._CHECKOUT_SOURCE_PATHS.items()}
        materials = {key: path.read_bytes() for key, path in runner._CHECKOUT_SOURCE_PATHS.items()}
        clean = subprocess.CompletedProcess([], 0, stdout="", stderr="")
        current = subprocess.CompletedProcess([], 0, stdout=revision + "\n", stderr="")
        with mock.patch.object(runner.subprocess, "run", side_effect=[current, clean]):
            runner._verify_checkout_sources(revision, paths, materials)

        with mock.patch.object(
            runner.subprocess,
            "run",
            return_value=subprocess.CompletedProcess([], 0, stdout="f" * 40, stderr=""),
        ):
            with self.assertRaisesRegex(runner.OperatorRunnerError, "checkout-revision-mismatch"):
                runner._verify_checkout_sources(revision, paths, materials)

        dirty = subprocess.CompletedProcess([], 0, stdout=" M tracked.py\n", stderr="")
        with mock.patch.object(runner.subprocess, "run", side_effect=[current, dirty]):
            with self.assertRaisesRegex(runner.OperatorRunnerError, "checkout-not-clean"):
                runner._verify_checkout_sources(revision, paths, materials)

        poisoned = dict(materials, coverage_source=b"caller-supplied old source")
        with mock.patch.object(runner.subprocess, "run", side_effect=[current, clean]):
            with self.assertRaisesRegex(
                runner.OperatorRunnerError, "checkout-source-material-mismatch:coverage_source"
            ):
                runner._verify_checkout_sources(revision, paths, poisoned)

    def test_source_preflight_failure_precedes_clock_and_handoff_creation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            current_revision = subprocess.run(
                ["git", "rev-parse", "--verify", "HEAD^{commit}"],
                cwd=runner._REPOSITORY_ROOT,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            config_path, _old_pin = self._config(root, b"{}")
            config = json.loads(config_path.read_bytes())
            config["source_revision"] = current_revision
            raw = canonical(config)
            config_path.write_bytes(raw)
            config_path.chmod(0o600)
            pin = digest(raw)
            current = subprocess.CompletedProcess([], 0, stdout=current_revision + "\n", stderr="")
            clean = subprocess.CompletedProcess([], 0, stdout="", stderr="")
            with (
                mock.patch.object(runner.subprocess, "run", side_effect=[current, clean]),
                mock.patch.object(runner.time, "monotonic", side_effect=AssertionError("clock must not start")),
                mock.patch.object(
                    runner.operator_handoff, "OperatorReceiptHandoff", side_effect=AssertionError("no handoff")
                ),
                mock.patch.object(runner, "_require_source_bootstrap"),
            ):
                with self.assertRaisesRegex(runner.OperatorRunnerError, "checkout-source-path-mismatch"):
                    runner.OperatorStageRunner.from_file(config_path, pin)
            self.assertFalse((root / "operator_handoff").exists())

    def test_confirmation_selects_only_its_explicit_stage_cohort(self):
        cohorts = {
            "stages": [
                {"stage": "development", "research_cases": [{"task_id": "dev"}], "navigation_targets": []},
                {
                    "stage": "confirmation",
                    "research_cases": [{"task_id": "confirm"}],
                    "navigation_targets": [],
                },
            ]
        }
        self.assertEqual(runner._stage_cohort(cohorts, "development")["research_cases"][0]["task_id"], "dev")
        self.assertEqual(runner._stage_cohort(cohorts, "confirmation")["research_cases"][0]["task_id"], "confirm")
        with self.assertRaisesRegex(runner.OperatorRunnerError, "configured-stage-cohort-not-unique"):
            runner._stage_cohort({"stages": [{"stage": "development"}]}, "confirmation")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config_path, _pin = self._config(root, b"{}")
            config = json.loads(config_path.read_bytes())
            config["stage_kind"] = "confirmation"
            config["forbidden_stage_uuids"].append(config["packet_stage_uuid"])
            raw = canonical(config)
            config_path.write_bytes(raw)
            config_path.chmod(0o600)
            with self.assertRaisesRegex(runner.OperatorRunnerError, "config-forbidden-stage-inventory-invalid"):
                runner._load_config(config_path, digest(raw))

    def test_answer_authority_allows_only_explicitly_opted_in_private_http(self):
        async def scenario(endpoint: str, allow: bool):
            temporary = tempfile.TemporaryDirectory()
            root = Path(temporary.name)
            secret = root / "answer.key"
            secret.write_text("synthetic-test-only\n", encoding="utf-8")
            secret.chmod(0o600)
            instance = object.__new__(runner.OperatorStageRunner)
            instance._test_temporary_directory = temporary
            instance.config = {"answer_endpoint": endpoint, "allow_trusted_private_http": allow}
            instance.private_paths = {"answer_api_key": str(secret)}
            instance.directories = {
                "lease_root": str(root / "leases"),
                "answer_archive": str(root / "archive"),
                "answer_results": str(root / "results"),
            }
            instance.plan = SimpleNamespace(
                stage_uuid=str(uuid.UUID(int=802)),
                source_revision="a" * 40,
                protocol_bytes=b"protocol",
                cohorts_bytes=b"cohorts",
            )
            instance.started_utc = datetime.now(timezone.utc)
            instance.deadline_utc = instance.started_utc + timedelta(hours=8)
            captured = {}

            async def request(scope, request_id, bindings):
                captured.update(scope=scope, request_id=request_id, bindings=bindings)
                return b"synthetic-receipt", "b" * 64

            instance._request = request
            return instance, captured

        async def run_private():
            instance, captured = await scenario("http://llm-svc:4000/v1", True)

            async def resolve(_url):
                return "http://10.0.0.8:4000/v1/chat/completions", "llm-svc:4000", "c" * 64

            with mock.patch.object(answer, "_resolve_private_http_destination", side_effect=resolve):
                authority = await instance._answer_authority(instance.plan, _tasks())
            self.assertTrue(authority["allow_trusted_private_http"])
            self.assertEqual(authority["endpoint"], "http://llm-svc:4000/v1")
            self.assertEqual(captured["bindings"]["endpoint_security_mode"], "trusted-private-http")
            self.assertEqual(captured["bindings"]["resolved_destination_sha256"], "c" * 64)

        async def run_public():
            instance, captured = await scenario("http://public.example/v1", True)
            with self.assertRaisesRegex(answer.AnswerExecutionError, "answer-plaintext-endpoint-not-permitted"):
                await instance._answer_authority(instance.plan, _tasks())
            self.assertEqual(captured, {})

        asyncio.run(run_private())
        asyncio.run(run_public())

    def test_wrong_answer_security_mode_is_rejected_before_lease(self):
        import pytest

        async def scenario(tmp_path):
            args = _stage_inputs(tmp_path)
            events = []

            class WrongModeVerifier:
                def verify(self, receipt_bytes, expected_receipt_sha256, bindings):
                    events.append("verify")
                    return answer.VerifiedAnswerPermit(
                        status="verified-admitted",
                        stage_uuid=bindings["stage_uuid"],
                        source_revision=bindings["source_revision"],
                        protocol_sha256=bindings["protocol_sha256"],
                        cohorts_sha256=bindings["cohorts_sha256"],
                        operation_manifest_sha256=bindings["operation_manifest_sha256"],
                        endpoint_sha256=bindings["endpoint_sha256"],
                        resolved_destination_sha256=bindings["resolved_destination_sha256"],
                        endpoint_security_mode="trusted-private-http",
                        operation_ids=tuple(bindings["operation_ids"]),
                        max_calls=answer.MAX_CALLS,
                        request_bytes_maximum=answer.MAX_REQUEST_BYTES,
                        response_bytes_maximum=answer.MAX_RESPONSE_BYTES,
                        receipt_sha256=expected_receipt_sha256,
                    )

            class CountLease:
                def consume_once(self, *_args, **_kwargs):
                    events.append("lease")

            args["permit_verifier"] = WrongModeVerifier()
            args["one_shot_lease"] = CountLease()
            with pytest.raises(answer.AnswerExecutionError, match="answer-permit-binding-mismatch"):
                await answer.execute_answer_stage(**args)
            self.assertEqual(events, ["verify"])
            self.assertEqual(list(args["lease_root"].iterdir()), [])

        with tempfile.TemporaryDirectory() as temporary:
            asyncio.run(scenario(Path(temporary)))

    def test_acquisition_plan_matches_fixed_cohort_engine_schedule(self):
        plan = plan_fixture()
        self.assertEqual(
            runner._build_acquisition_plan(plan.stage_uuid, list(plan.research_cases), list(plan.navigation_targets)),
            plan.acquisition_plan_bytes,
        )

    def test_answer_authority_preserves_configured_endpoint_binding(self):
        # The executor hashes the configured endpoint string, then canonicalizes
        # it for dispatch. Base URLs and /v1 URLs must therefore remain the
        # value returned in authority, rather than the appended request URL.
        for endpoint in (
            "https://model.example",
            "https://model.example/v1",
            "https://model.example/v1/chat/completions",
        ):
            with self.subTest(endpoint=endpoint), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                secret = root / "answer.key"
                secret.write_text("synthetic-test-only\n", encoding="utf-8")
                secret.chmod(0o600)
                instance = object.__new__(runner.OperatorStageRunner)
                instance.config = {"answer_endpoint": endpoint}
                instance.private_paths = {"answer_api_key": str(secret)}
                instance.directories = {
                    "lease_root": str(root / "leases"),
                    "answer_archive": str(root / "archive"),
                    "answer_results": str(root / "results"),
                }
                instance.plan = SimpleNamespace(
                    stage_uuid=str(uuid.UUID(int=801)),
                    source_revision="a" * 40,
                    protocol_bytes=b"protocol",
                    cohorts_bytes=b"cohorts",
                )
                instance.started_utc = datetime.now(timezone.utc)
                instance.deadline_utc = instance.started_utc + timedelta(hours=8)
                captured = {}

                async def request(scope, request_id, bindings):
                    captured.update(scope=scope, request_id=request_id, bindings=bindings)
                    return b"synthetic-receipt", "b" * 64

                instance._request = request
                authority = asyncio.run(instance._answer_authority(instance.plan, _tasks()))
                self.assertEqual(authority["endpoint"], endpoint)
                self.assertEqual(captured["scope"], "answer-execution")
                self.assertEqual(
                    captured["bindings"]["endpoint_sha256"],
                    answer._sha(
                        answer._canonical(
                            {
                                "endpoint": endpoint,
                                "security_mode": "https-required",
                                "resolved_destination_sha256": answer._sha(
                                    answer._canonical({"mode": "https-required"})
                                ),
                            }
                        )
                    ),
                )
                self.assertEqual(
                    answer._chat_url(authority["endpoint"])[0], "https://model.example/v1/chat/completions"
                )


if __name__ == "__main__":
    unittest.main()
