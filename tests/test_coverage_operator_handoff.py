from __future__ import annotations

import base64
import hashlib
import json
import stat
import tempfile
import threading
import time
import unittest
import uuid
from pathlib import Path

from scripts import coverage_jev_execution as jev_execution
from scripts import coverage_operator_handoff as handoff
from scripts import coverage_selector_input_map
from tests.test_coverage_selector_input_map import _materials


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def selector_permit_batch(tmp_path: Path) -> bytes:
    """Build a full-shaped batch from the registered operation schedule fixture."""
    tmp_path.mkdir(mode=0o700)
    operation_materials: dict[str, object] = {}
    inputs = _materials(tmp_path)
    inputs["operation_materials"] = operation_materials
    map_bytes = coverage_selector_input_map.build_selector_input_map(**inputs)
    map_doc = json.loads(map_bytes)
    prepared = inputs["prepared"]
    rows = []
    for operation in map_doc["operations"]:
        permit = jev_execution._canonical(
            {
                "schema": jev_execution.PERMIT_SCHEMA,
                "status": "verified-admitted",
                "stage_uuid": prepared.stage_uuid,
                "operation_id": operation["operation_id"],
                "parser_mode": operation["parser_mode"],
                "source_revision": prepared.source_revision,
                "protocol_sha256": prepared.pins["protocol"],
                "source_closure_sha256": prepared.pins["qualified_source_closure"],
                "operation_input_sha256": operation["operation_input_sha256"],
                "request_body_sha256": operation["request_body_sha256"],
            }
        )
        rows.append(
            {
                "operation_id": operation["operation_id"],
                "permit_base64": base64.b64encode(permit).decode("ascii"),
                "permit_sha256": digest(permit),
            }
        )
        if len(permit) > handoff.MAX_RECEIPT_BYTES:
            raise AssertionError("fixture must stay inside the individual-permit cap")
    return canonical(
        {
            "schema": "coverage-selector-permit-batch/1",
            "stage_uuid": prepared.stage_uuid,
            "selector_input_map_sha256": hashlib.sha256(map_bytes).hexdigest(),
            "operations": rows,
        }
    )


class OperatorReceiptHandoffTests(unittest.TestCase):
    def test_waits_for_separate_operator_receipt_and_pin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8001))
            bindings = {"source_revision": "a" * 40, "protocol_sha256": "b" * 64, "input_sha256": "c" * 64}
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="selector-admission",
                            request_id="late-map-1",
                            bindings=bindings,
                            deadline_monotonic=time.monotonic() + 2,
                        )
                    )
                except Exception as exc:  # surfaced in assertion below
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "selector-admission" / "requests" / "late-map-1.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            self.assertTrue(request_path.is_file())
            envelope = json.loads(request_path.read_bytes())
            self.assertEqual(envelope["bindings"], bindings)
            self.assertEqual(envelope["request_sha256"], digest(canonical(bindings)))
            self.assertEqual(stat.S_IMODE(request_path.stat().st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(root.stat().st_mode), 0o700)
            receipt = canonical({"externally_issued": True, "opaque": "receipt-bytes"})
            response = root / stage / "selector-admission" / "responses" / "late-map-1.receipt"
            pin = root / stage / "selector-admission" / "pins" / "late-map-1.sha256"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin.write_text(digest(receipt) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_selector_batch_accepts_exact_39_operation_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8005))
            receipt = selector_permit_batch(Path(temporary) / "fixture")
            self.assertGreater(len(receipt), handoff.MAX_RECEIPT_BYTES)
            self.assertLessEqual(len(receipt), handoff.MAX_SELECTOR_PERMIT_BATCH_BYTES)
            batch = json.loads(receipt)
            self.assertEqual(len(batch["operations"]), 39)
            permits = [json.loads(base64.b64decode(row["permit_base64"])) for row in batch["operations"]]
            self.assertEqual(
                [row["operation_id"] for row in batch["operations"]],
                [permit["operation_id"] for permit in permits],
            )
            self.assertTrue(all(permit["schema"] == "coverage-jev-operation-permit/1" for permit in permits))
            self.assertTrue(all(permit["status"] == "verified-admitted" for permit in permits))
            self.assertTrue(all(len(permit) == 10 for permit in permits))
            self.assertTrue(all(permit["parser_mode"] in {"original-v1", "coverage"} for permit in permits))
            result: list[object] = []

            def run_request():
                try:
                    result.append(
                        exchange.request(
                            stage_uuid=stage,
                            scope="selector-operation-permits",
                            request_id="batch",
                            bindings={"operation_count": 39, "selector_input_map_sha256": "f" * 64},
                            deadline_monotonic=time.monotonic() + 2,
                        )
                    )
                except Exception as exc:  # surfaced in assertions below
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            request_path = root / stage / "selector-operation-permits" / "requests" / "batch.json"
            for _ in range(100):
                if request_path.exists():
                    break
                time.sleep(0.005)
            response = root / stage / "selector-operation-permits" / "responses" / "batch.receipt"
            pin = root / stage / "selector-operation-permits" / "pins" / "batch.sha256"
            response.write_bytes(receipt)
            response.chmod(0o600)
            pin.write_text(digest(receipt) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(result, [(receipt, digest(receipt))])

    def test_selector_batch_over_cap_is_terminal_and_not_reissued(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8006))
            kwargs = {
                "stage_uuid": stage,
                "scope": "selector-operation-permits",
                "request_id": "batch",
                "bindings": {"operation_count": 39},
                "deadline_monotonic": time.monotonic() + 2,
            }
            result: list[object] = []

            def run_request():
                try:
                    result.append(exchange.request(**kwargs))
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            response = root / stage / "selector-operation-permits" / "responses" / "batch.receipt"
            for _ in range(100):
                if response.parent.is_dir():
                    break
                time.sleep(0.005)
            oversized = b"x" * (handoff.MAX_SELECTOR_PERMIT_BATCH_BYTES + 1)
            response.write_bytes(oversized)
            response.chmod(0o600)
            pin = root / stage / "selector-operation-permits" / "pins" / "batch.sha256"
            pin.write_text(digest(oversized) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(len(result), 1)
            self.assertIsInstance(result[0], handoff.OperatorHandoffError)
            self.assertEqual(str(result[0]), "handoff-response-over-cap")
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-request-already-created-no-resume"):
                exchange.request(**kwargs)

    def test_other_receipt_scopes_keep_the_original_cap(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8007))
            kwargs = {
                "stage_uuid": stage,
                "scope": "selector-map-admission",
                "request_id": "map",
                "bindings": {"selector_input_map_sha256": "a" * 64},
                "deadline_monotonic": time.monotonic() + 2,
            }
            result: list[object] = []

            def run_request():
                try:
                    result.append(exchange.request(**kwargs))
                except Exception as exc:
                    result.append(exc)

            worker = threading.Thread(target=run_request)
            worker.start()
            response = root / stage / "selector-map-admission" / "responses" / "map.receipt"
            for _ in range(100):
                if response.parent.is_dir():
                    break
                time.sleep(0.005)
            oversized = b"x" * (handoff.MAX_RECEIPT_BYTES + 1)
            response.write_bytes(oversized)
            response.chmod(0o600)
            pin = root / stage / "selector-map-admission" / "pins" / "map.sha256"
            pin.write_text(digest(oversized) + "\n", encoding="ascii")
            pin.chmod(0o600)
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
            self.assertEqual(len(result), 1)
            self.assertIsInstance(result[0], handoff.OperatorHandoffError)
            self.assertEqual(str(result[0]), "handoff-response-over-cap")

    def test_bad_pin_is_terminal_and_request_cannot_be_reissued(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8002))
            response = root / stage / "capture" / "responses" / "receipt-1.receipt"
            pin = root / stage / "capture" / "pins" / "receipt-1.sha256"
            (root / stage / "capture" / "responses").mkdir(parents=True, mode=0o700)
            (root / stage / "capture").chmod(0o700)
            (root / stage).chmod(0o700)
            (root / stage / "capture" / "pins").mkdir(mode=0o700)
            response.write_bytes(b"receipt")
            response.chmod(0o600)
            pin.write_text("0" * 64, encoding="ascii")
            pin.chmod(0o600)
            kwargs = {
                "stage_uuid": stage,
                "scope": "capture",
                "request_id": "receipt-1",
                "bindings": {"manifest_sha256": "a" * 64},
                "deadline_monotonic": time.monotonic() + 1,
            }
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-pin-mismatch"):
                exchange.request(**kwargs)
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-request-already-created-no-resume"):
                exchange.request(**kwargs)

    def test_timeout_leaves_request_and_never_claims_receipt(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "handoff"
            exchange = handoff.OperatorReceiptHandoff(root, poll_interval=0.02)
            stage = str(uuid.UUID(int=8003))
            with self.assertRaisesRegex(handoff.OperatorHandoffError, "handoff-receipt-timeout-no-resume"):
                exchange.request(
                    stage_uuid=stage,
                    scope="answer",
                    request_id="answer-1",
                    bindings={"operation_manifest_sha256": "d" * 64},
                    deadline_monotonic=time.monotonic() + 0.07,
                )
            self.assertTrue((root / stage / "answer" / "requests" / "answer-1.json").is_file())

    def test_scope_request_and_deadline_are_strict(self):
        with tempfile.TemporaryDirectory() as temporary:
            exchange = handoff.OperatorReceiptHandoff(Path(temporary) / "handoff")
            base = {
                "stage_uuid": str(uuid.UUID(int=8004)),
                "scope": "answer",
                "request_id": "safe-id",
                "bindings": {"x": 1},
                "deadline_monotonic": time.monotonic() + 1,
            }
            for change in (
                {"scope": "../escape"},
                {"request_id": "../escape"},
                {"deadline_monotonic": time.monotonic() + handoff.MAX_WAIT_SECONDS + 1},
                {"bindings": {"bad": float("nan")}},
                {"bindings": {"api_key": "never-persist"}},
            ):
                with self.subTest(change=change), self.assertRaises(handoff.OperatorHandoffError):
                    exchange.request(**(base | change))


if __name__ == "__main__":
    unittest.main()
