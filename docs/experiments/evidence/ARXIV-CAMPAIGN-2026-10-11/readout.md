# Campaign readout — 2026-10-11

## Outcome and ownership

Five meta-search candidates were measured on24 externally labeled development queries (12 SciFact,12 NFCorpus) through real SearchService with frozen benchmark-local title/fulltext resources. None cleared all registered advancement gates; no untouched confirmation cohort was opened, no compound experiment was run, and no SlopSearX feature request was warranted. These results reject or leave inconclusive the particular bounded candidates, not the papers or all future approaches.

Three planning/orchestration candidates were routed before execution to Grok-to-Crawl-X, along with wrapper-owned answer/page-cache tiers and generated evidence facets. Requests:#444 rewriting,#445 evidence-conditioned follow-ups,#446 stopping,#447 answer/page freshness,#448 generated facets. Those requests are proposals, not completed experiments or permission to implement/deploy the wrapper.

| Core trial | Primary result | Quality/guard finding | Decision |
|---|---|---|---|
| EXP-113 fixed RRF k20 versus k60 | nDCG@10 delta0.000; adjusted interval[0,0] | Identical primary quality on both datasets; membership/provenance preserved | not-supported |
| EXP-112 resource selection | Mean resource-call saving50%; interval[50%,50%] | nDCG+.06825; interval[−.02883,+.16876] fails −.02 noninferiority | inconclusive |
| EXP-114 selective reranking | Mean per-query scorer-token saving29.17%; interval[8.33%,54.17%] | nDCG−.03250; SciFact−.05110 fails per-domain guard | not-supported |
| EXP-115 adaptive depth | Mean per-query scorer-token saving14.25%; interval[3.93%,26.65%] | Below25% minimum; nDCG interval lower bound−.02082 also misses−.02 | not-supported |
| EXP-117 deterministic redundancy view | Relevant-document recall@10 delta0.000; interval[0,0] | nDCG−.00511; no required coverage gain | not-supported |

EXP-118 provided measurement readiness, not an improvement experiment. The current datasets cover two biomedical retrieval tasks, not all project query domains. EXP-116 search-response freshness is blocked at dataset eligibility: no real temporal recurrence/capture/answer-change cohort. Static qrels and fake TTL clocks cannot measure its proposed effect. No temporal provider calls or serving changes.

## Sequencing and interactions

All single-component comparisons retained their shipped/pinned individual baseline; a winning earlier stage was not silently incorporated into another baseline. Confirmation and combined four-arm studies require successful individual advancement and fresh task families. None advanced, so testing compounds now would compound unqualified assumptions.

One explanatory finding: the already-shipped optional Jev scorer improved this pool's nDCG by+.12917 over no-rerank (adjusted interval[+.03164,+.25661]). This is a secondary benchmark observation, not a new feature and not a rescue for the failed selective/depth primaries. It explains why skipping/decreasing scoring can lose relevance. Future resource selection needs a larger preregistered cohort to resolve noninferiority; do not claim the mean alone justifies rollout. Future diversity needs independent facet labels before subtopic claims.

## Evidence limits and deviations

- Benchmark-local BM25 title/fulltext resources are controlled inputs, not captured heterogeneous upstream engines. No general federated relevance, human/agent answer quality, upstream availability, live latency or production adoption conclusion.
- Queries were hash-selected before metric inspection. Corpus metadata containing source support labels was excluded from candidate state. External qrels were loaded only for analysis; confirmation texts/qrels remained unopened. All24 planned queries remained in denominators. There was no candidate tuning after outcomes.
- Full denominators include relevant corpus documents not retrieved; unjudged items follow benchmark zero-grade conventions. Public benchmark judgments can be incomplete and training contamination remains possible. Two datasets and24 queries limit power/generalization. Bootstrap is task-level resampling sensitivity, not proof of representative production traffic.
- Original fixture qualification rejected unregistered temporary engines. A premature run/analysis found no prepared input. Both failed setup paths are retained, with zero provider/candidate/quality outputs. Normal ephemeral registration repaired qualification without changing live policy.
- EXP-112's initial post-call assertion incorrectly expected selected-scope results to retain unqueried-engine provenance. Corrected before gold/metric inspection; the valid398-token response was retained and replayed without retry, then23 remaining planned calls executed under the original clock. Original worker/start/response remain in evidence. This repaired an invalid fixture invariant, not a product policy gate.
- Baseline20 scorer results were acquired once and reused for selective/depth comparisons; only7 planned depth10 calls were newly invoked. This controls baseline stochasticity but does not measure contemporaneous end-to-end p95 savings. Provider round-trip times are retained; token cost is primary.
- Mean per-query savings differs from aggregate token percentage; both are retained. Provider spend for experimentation includes baseline acquisition, so the experiment itself does not save money. All55 provider responses valid; no retries or paid search calls.
- Exact-title/navigation guards were not supplied by these sampled datasets. No feature request can follow without separately registered actual-engine checks. Hard scope/membership/provenance/serialization guards run during replay; they do not establish all portal or policy contracts. An initial pytest command named nonexistent test_filters.py and exited4 before tests; no passing-suite claim. No product change or implementation PR was made, so no implementation regression gate was required or bypassed.
- Source text remains in pinned external datasets and transient local input. Durable ordered feed IDs, hashes and reconstruction code support replay without redistributing corpus abstracts. Original BM25 set-iteration floating-point order is frozen by the retained feed index; exact replay must use that index. Sorted-term preparation in portable runner is not a rerun or change to the measured inputs.

## Reproduction and preservation

Download official BEIR scifact/nfcorpus archives into a local data directory and verify manifest archive/file hashes. Use the frozen prepared-development-index.json to reconstruct feed projections from corpus IDs and verify title/content hashes. Recreate metrics from saved rows and external test qrels using the preserved analysis routine; provider responses must be replayed offline, never re-requested for reproduction. Receipt IDs, selected resource decisions and exact score responses retain all original attempts. No secret headers or credential values are published.

Worker revisions, raw attempt/response journals, all24 rows per candidate, analyzed rows, summaries, component/dependency hashes, source/provider provenance, dataset pins, fixture failures, ownership links, budgets and this readout are durable. Confirmation was not invoked. Status remains: development screens complete; wrapper requests submitted; temporal core experiment blocked pending real data. No background scheduler is configured.

## Cost accounting

{
  "provider": "typesafe",
  "model": "jev-1.13.0",
  "attempts": 55,
  "valid": 55,
  "input_tokens": 327362,
  "output_tokens": 8976,
  "unknown_usage_attempts": 0,
  "price_usd_per_million_input": 0.042,
  "paid_search_calls": 0,
  "known_estimated_usd": 0.013749204000000001,
  "reserved_upper_usd": 0.013749204000000001
}

Documentation persistence: [SlopSearX PR#781](https://github.com/magnus919/SlopSearX/pull/781). Verify current merge state on GitHub; no implementation PR exists.
