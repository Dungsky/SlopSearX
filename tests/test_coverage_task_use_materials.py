"""Cross-file source binding for the current prospective research method."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


def test_current_task_use_method_binds_checked_out_candidate_source():
    root = Path(__file__).parents[1]
    materials = root / "docs/experiments/evidence/coverage-first-study/research-task-use-method-materials"
    protocol = json.loads((materials / "protocol.json").read_bytes())
    candidate_sha = hashlib.sha256((root / "scripts/intent_ranking_coverage.py").read_bytes()).hexdigest()
    assert protocol["candidate_source_sha256"] == candidate_sha
