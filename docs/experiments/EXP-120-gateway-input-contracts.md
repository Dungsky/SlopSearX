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

## Readout — primary supported; delivery inconclusive/blocked

Frozen primary schema equality **0/3 → 3/3**; complete current catalog input
schema equality **0/38 → 38/38**. Both successful capability and invalid-intent
controls have exactly equal complete upstream and gateway response objects
across arms, including text/error flags. One valid frozen acquisition per arm;
no primary retries, exclusions, threshold or worker changes. This establishes
current input-contract preservation, not agent-task or production benefit.

Candidate `68ca36d6ac8043b57da99f1c4d155171cea5159c`, registration `f0927d4`,
baseline `309384f`. [PR #802](https://github.com/magnus919/SlopSearX/pull/802)
remains **unmerged**. All applicable exact-head CI passed, including both full
Python suites and portal/Valkey/API/security/build gates. The ordinary merge
was rejected by base policy; the earlier automated changes-requested review
remains. No admin override, dismissal or protection modification occurred.

The first automated finding (collector exit zero) was addressed with a separate
explicit offline verifier without rewriting the frozen collector. The next
Droid concern (structured-only controls) was addressed using already-retained
full responses. Baseline fails the decision gate, candidate passes its schema/
response checks, nine mutations fail even under -O. Original negative reviews
and resolutions are retained; none is erased or treated as approval.

Final Droid review identified an unclosed catalog names/description guard.
A separate discovery-only regression was attempted before the45-minute cutoff.
Its direct/upstream-vs-gateway comparison failed:19 descriptions differed;
shown examples include indentation/whitespace normalization. That comparison
is not the originally intended baseline-vs-candidate unchanged-metadata check,
and does not establish a candidate-introduced regression. The original collector
did not retain descriptions, so unchanged before/after descriptions could not
be independently demonstrated within this cycle. No descriptor threshold was
relaxed and no passing metadata verdict is claimed. Additional analyzer edits
then failed because the expected metadata artifact had not been produced;
these uncommitted edits and all failed outputs are retained as unsuccessful work,
not authoritative protocol amendments. Earlier provisional positive runner
review is superseded for delivery by this unresolved guard.

Stop: registered experiment cutoff2026-10-12T13:44:23Z. No new measurement after
that cutoff; documentation persistence continues separately. Overall outcome is
**inconclusive for implementation delivery**, despite supported schema fidelity.
The candidate is retained pending prospective guard closure and normal review
resolution; it is not adopted, deployed or counted as a delivered improvement.
Do not rerun the frozen primary, reuse an exhausted clock or infer baseline
metadata equality from source plausibility. A fresh bounded guard-closure plan
is required for further measurement.

Local evidence:137 contract tests,24 final gateway tests, three browser tests,
mypy121 files and changed-file hooks passed. Eleven isolated transport modules
passed; remainder2798 passed/58 skipped/44 subtests with two failures. Both
active-environment preflight/inventory failures reproduce unchanged baseline;
the local full suite is not counted passing. Unrelated all-file formatter edits
were retained/restored. AST graph updated. Both main replay arms emitted the
retained cancel-scope shutdown diagnostic, not a lifecycle-improvement claim.

Pending docs registration [#800](https://github.com/magnus919/SlopSearX/pull/800)
was verified merged as `309384f` during this cycle, with a signed skip-CI marker
and no changes to its static materials or execution authority. Ranking #516
remains unqualified; none of its terminal phases was resumed.

Experiment provider/engine/Jev calls zero. CI reviewer observations are retained
separately; their complete cost and subscription usage are unknown. Actual time
accounting records measurement cutoff and later readout separately. Release
status unknown; no runtime merge, deployment, alerting or production SLO work.
