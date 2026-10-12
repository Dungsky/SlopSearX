# EXP-120 — preserve remote input contracts

## Frozen registration

- Cycle start 2026-10-12T12:59:23Z; deadline 13:44:23Z. Owner daily runner; issue #801; default45 minutes. Zero provider/engine/Jev calls.
- Baseline 309384ff92674f901d176b38e6b291f11717acb3. Current SlopSearX tool catalog over authenticated HTTP, state_factory injection supplies fake engines/in-memory store. No production state touched.
- Consequential problem: gateway users receive incomplete definitions for specialist and planning inputs (string array items, object array items, nullable typed fields). Agents need those contracts to formulate valid calls. This measures protocol preservation, not agent-task success or relevance.
- Primary: exact input-schema equality for slopsearx_search_targeted, slopsearx_search_science and slopsearx_plan_research. Baseline0/3. Practical minimum3/3, +100pp: all three contracts fully preserved, not just titles or top-level type tags.
- Candidate: register FunctionTool with a deep copy of remote input schema as parameters. Keep existing generated callable, argument handling, output conversion, auth/session and upstream validation. Discovery metadata is preserved; no new local validation promise. Other tool metadata/resources/output schemas outside scope.
- Guardrails: all38 current catalog input schemas equal; catalog names/descriptions unchanged; successful capability call and invalid-intent error unchanged; no engine/provider dispatch; schema source cannot be mutated through proxy registration; typed nested/nullable/enumerated schema regression; old direct helper use remains valid; wrong-token and pagination guards remain green; portal contracts/browser and normal CI/review.
- Sample/stopping: frozen public worker once per arm, baseline ready already acquired. No primary rerun/exclusion/tuning. Fixed exhaustive current catalog; uncertainty is version/catalog-specific, production/task gain unmeasured. Retain shutdown diagnostics as a fixture limitation if present. Stop on any primary/guard failure, preserve outcome.
- Reproduction: PYTHONPATH=. .venv/bin/python docs/experiments/evidence/EXP-120/worker.py.txt /private/tmp/exp120-new-output. Requires recorded Python3.12/FastMCP3.4.7/MCP1.29.0; two disposable loopback HTTP servers, static fixture token.
- Portal: gateway-only discovery schema, shared service/HTTP response and HTML unaffected; required portal CI still gates delivery.
- Supported delivery requires all gates plus required exact-head CI and substantive Droid-or-runner review; no deploy/SLO/alerts.

## Opportunity and recent outcome review

Ranking #516 has advanced through new setup fixes and static registration #800
(now merged), but requires independent stage admission; no quality result yet.
Do not retry its terminal phases. Recent acquisition/startup/capture failures
remain separate immutable records. Latest completed numbered evaluations
EXP-119,117,115,114,112,113,118 give five rejected prototypes, one inconclusive
screen and one measurement prerequisite; none supported product adoption.
EXP-108/107/106 delivered protocol/recovery fixes with scoped deterministic
benefit. Schema loss is a current, directly reproducible defect affecting every
advertised tool, with affordable decisive measurement; it outranks speculative
ranking/host UI work lacking admission or independent host evidence today.

Frozen worker SHA256 `37a91f8e3d98596876f22ef24fcfce6f7f6f39a0e2a3fb8467b44698d45ae41b`.

## Review clarification — measurement and decision are separate

The frozen worker is a measurement collector; its process exit is not the
experiment verdict (the expected deficient baseline also exits zero). The
runner originally checked the registered counts and raw equality results.
Review requested an explicit executable gate: `verify.py.txt` now applies the
unchanged frozen predicates offline to retained output and returns nonzero on
failure. It accepts the original candidate record and rejects seven deliberately
corrupted records under `verify-tests.py.txt`. These are analyzer regressions,
not new primary observations. No worker, corpus, threshold, outcome or primary
replay was changed or repeated. Reproduction must run both collector and verifier.

Droid additionally requested full control checks beyond structured content.
Offline comparison of the already-retained baseline/candidate records shows
both upstream and gateway responses exactly equal for both controls, including
text and error flags. The verifier enforces that full equality and rejects nine
mutants, including text/error-flag changes. Explicit ValueError checks retain
enforcement under Python optimization. No new primary acquisition or threshold
change occurred.
