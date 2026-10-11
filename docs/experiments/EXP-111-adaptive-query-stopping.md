# EXP-111: adaptive-query-stopping

## Registration

State: planned; freeze activation before this candidate's work. Date:2026-10-11; user-authorized bounded campaign. Paper 3: https://arxiv.org/abs/2601.21208. Related implementation issue: none; create only after supported confirmation.

Hypothesis: Stopping after the second query when it adds no new top-ten document IDs saves at least 20% logical retrieval requests with nDCG@10 noninferiority within 0.02 versus all three frozen rewrites.

Primary: request saving >=20%. Candidate: original then two fixed rewrites from EXP-109; observable membership-only stop rule, never relevance labels. No RL reproduction claim.

Beneficiary: callers seeking useful results with bounded requests/latency. Current project seams and historical counterevidence are in the linked [campaign](ARXIV-CAMPAIGN-2026-10-11.md).

## Shared frozen controls

Baseline main 178bc43bad357a7d9bb2177415cd75807bbb7c3d; explicitly requested ten-paper campaign overrides recurring cadence for this sequence only. One active trial at a time, at most 45 minutes per trial. Whole-campaign paid budget $10; use only project-authorized Typesafe credential, no paid search planned. Pin jev-1.13.0 after current official model/price verification; maximum 2 million input tokens across campaign at verified $0.042/M ($0.084), at most 150 provider attempts, no retries, 1-second HTTP timeout for reranking and routing, unknown failed-call usage reserved at 64k tokens per failed call. Stop on price/model/usage mismatch or total budget. Provider errors are missing measurement, not irrelevant results. No credential printing or unrelated credential reuse.

Pinned manifests under evidence/ARXIV-CAMPAIGN-2026-10-11/manifest.json. Dataset archives downloaded from official BEIR distribution before metrics or candidates. SHA256 files and query IDs frozen. 12 development and 12 disjoint confirmation queries per dataset chosen by hashed IDs; 24 independent query units per phase. Development inputs may inform feasibility only; confirmation query text and qrels must not be used to tune candidates. Evaluation denominators use all gold relevant documents, including candidates absent from returned feeds. Restrict claims to these static public corpora; incomplete benchmark judgments and two biomedical domains limit transfer. No live-web or end-to-end answer-quality claim.

Retrieval fixture: fixed BM25 k1=1.2,b=.75, lowercase ASCII-alphanumeric tokenization, separate title-only and title+text indexes, top20 per resource. Ties by document ID. These are benchmark resources, not fabricated captured upstream outputs. Replay these through actual SearchService and shipped fusion/presence, explicit scope, no cache/suggestions/Jev unless the trial adds it. Candidate views preserve full result membership, provenance, tiers and JSON round-trip. Check policy refusal, filter truthfulness, cache/snapshot guard boundaries with relevant existing contract tests before any feature request.

Query-paired bootstrap 20,000 samples, seed20261011, two-sided99.5% intervals (ten-comparison family bound). A quality claim requires lower bound clearing the minimum useful effect; savings claims require savings lower bound clearing threshold and quality lower bound >=−.02. Each dataset mean must satisfy quality noninferiority >=−.02; no loss of benchmark exact-match controls. Missing ≥10% required provider responses makes measurement inconclusive; any immutable contract failure rejects candidate. No exclusions or tuning after outcomes. Development success alone cannot trigger a feature request. Only confirmation plus all guards passing may support a narrowly scoped request; evidence must state proxy limits and live qualification needed. An underpowered result is inconclusive, not disproved. Candidate failing completed fixed gates is not-supported.

Exact run commands (inert harness created only after signed registration): `PYTHONPATH=. /Volumes/tank01/magnus/git/SlopSearX/.venv/bin/python docs/experiments/evidence/ARXIV-CAMPAIGN-2026-10-11/runner.py.txt prepare`; then `... runner.py.txt run EXP-NNN development`; analyze with `... runner.py.txt analyze EXP-NNN development`; confirmation is invoked once only if development advancement passes. All commands use the pinned local data directory /private/tmp/slopsearx-arxiv-campaign/data. Captured code is inert .py.txt, responses/attempts are append-only, evidence and SHA256s are retained under the campaign directory. Inputs missing a task-appropriate corpus get a retained eligibility receipt; no replacement measurement of convenience. Existing #516 frozen cohort and terminal stopping rules are untouched.

Portal impact: no production/backend/browser contract changes. Any later implementation needs applicable portal CI and API/MCP/cache/snapshot coverage. This campaign ends in feature requests only for supported gains, not implementation or deployment.

## Ownership correction before execution

Maintainer clarified meta-search/wrapper boundary after planning registration. No candidate ran. This record is routed to Grok-to-Crawl-X; do not execute orchestration in SlopSearX. Request: https://github.com/magnus919/groktocrawl-x/issues/446 . Original hypothesis preserved as history; wrapper issue specifies the prospective experiment.
