# EXP-119 readout — source-selection iteration

## Outcome

**not-supported at development; no confirmation invoked.** The registered metadata-only selector did not clear the useful-quality threshold over the strong fixed control. All20 original topics and the2 separately registered stability operations completed; all88 provider responses were valid. No source result, failed call or topic was retried/replaced/excluded. The30-topic confirmation cohort remains uninvoked, so no heldout generalization or project deployment conclusion follows.

| Measure | Value |
|---|---:|
| Candidate normalized graded precision at5 | 0.279853 |
| Development-trained fixed5-source control | 0.805390 |
| Metadata BM25 control | 0.020332 |
| Primary minimum mean gain over BOTH controls | −0.525537 |
| Paired-bootstrap95% interval | [−0.628428,−0.414095] |
| Required interval lower bound | +0.05 |
| Stability overlaps (identical/rotated packing) | 0.8 /1.0 |
| Selected resources without official judgments | 9% |
| Maximum observed total query routing wall time | 773.198ms |

The fixed control was optimized on these same20 development topics. Its development score is optimistic; it is not an unbiased estimate of heldout performance and the conditional bootstrap does not account for fitting that control. The registered development gate intentionally demanded competition with this strong cheap policy before heldout spend. This result rejects this prototype under that gate; it does not prove fixed routing is universally better or that ReSLLM/SLAT cannot work.

All current prototype guards passed: complete148-source score sets, selected5 allowed unique IDs, finite pinned-model typed outputs with observed usage, two stability probes,≤2s per-query routing wall time,≤1s each provider request, no source URL dispatch, no source/gold grade leak into requests, and9% unjudged selections below20% cap. Official unjudged-zero conventions and historical human judgments remain imperfect. These are research-selector guards, not all API/MCP/portal release gates.

## Earlier result revisited without changing it

The retrospective EXP-112 fixed-control audit ran through the existing SearchService fixture and reproduced all24 previous effective result lists using the constant fulltext resource alone. The original model chose fulltext17 times and selected nothing7 times, where fallback also chose fulltext. No new provider calls or relevance grades were used. Exact result-list identity means the earlier mean quality gain can arise without query adaptation. Its original inconclusive outcome and evidence remain untouched.

This is an explanatory audit after observing the original decisions, not a fresh preregistered hypothesis or confirmation result. It removes the basis for calling the EXP-112 gain evidence of adaptive routing value.

## What changes in research selection

1. Require strong cheap fixed/source-prior controls before crediting a model-assisted routing gain. Comparing only with querying every resource confounds adaptation with dropping a weak resource.
2. Prefer papers that learn from actual retrieved evidence, collection sampling, overlap and empirical source yield. Metadata/brand/vertical fit alone did not predict normalized top-ten resource quality adequately here.
3. Preserve a strong general-search baseline and measure incremental specialist value, essential-source misses and useful visible evidence. Do not assume the most topical resource adds the most evidence.
4. Seek modern SlopSearX captures alongside historical federated benchmarks. Nguyen et al.'s collection-size study explicitly warns about skewed sizes and general-web-query bias; a FedWeb win or failure may reflect that workload rather than current user research needs. Do not transfer historical Google Blogs/Kidrex/Ask/YouTube/AOL Video control choices into current configuration.
5. Reranking-confidence research remains a separate later opportunity; no new reranking or wrapper study was run. Query rewriting, evidence-conditioned planning and stopping remain in Grok-to-Crawl-X issues444–446, with cache/facet requests447–448 unchanged.

## Evidence and scope

The corpus contains50 judged historical topics and149 listed resource records. Virtual BigWeb (no URL) was excluded by the frozen metadata-only eligibility rule, leaving148. Official resource qrel IDs all exist in metadata;4286 topic/resource pairs have no duplicates. All query texts, resource metadata, judgments and metric documentation come from pinned NIST files. Two grade excerpts visible during source discovery were forced into development before scoring. Confirmation labels were not used to fit prompts, controls or decisions; public benchmark contamination in model training remains unknown.

The primary is the official linear normalized graded-precision resource metric. Scores0–1000 were not exponentiated as ordinary ordinal document grades. It measures judged quality of selected historical sources, not current live returned-card relevance, answer quality or end-to-end search latency. Source-level priors differ from modern adapter credentials, costs, policy and health. Historical resource IDs are explicitly refused by the current adapter policy in qualification. No current engine was dispatched or changed.

Initial source download with Python's default user agent returned403 before registration. An identified research user agent retrieved the same public official files; that preflight failure is retained and was not a scientific/model retry. The new worker passed12 offline qualification checks before controls/model calls. The24-case earlier fixed-control audit also passed. No production code/configuration/dependency/workflow files changed; no implementation CI or portal gate was required for documentation-only evidence.

Signed registration81f3026; qualified worker/control freeze aa5a947. All provider score operations and analyses followed the fixed plan. No thresholds were relaxed; no repeated test to search for a favorable draw; no confirmation or compound stage invoked. Registration permitted a maximum208 operations and2M known/reserved input tokens within the continuing$10 allowance; actual88 operations fit the bound.

## Reproduction and preservation

Evidence includes the original metadata/numeric judgments, official file hashes, query split, exact selected controls/training scope, registered worker, all88 request hashes/responses with usage/timing,20 original rows plus2 stability rows, analysis rows, retrospective fixed-control matches, setup failure and this readout. Request bodies are deterministically reconstructed from metadata/query bytes and worker code; credentials/headers are never retained. Reanalysis uses saved values/labels, not provider re-invocation. Do not run score for reproduction: original attempts are single-invocation history.

No feature request was created: the candidate failed development, and even historical source-quality support would require separately registered current-adapter/result-quality confirmation before implementation. No runtime/default/deployment changes. No background scheduler is configured.

## Accounting

{
  "provider": "typesafe",
  "model": "jev-1.13.0",
  "attempts": 88,
  "valid_responses": 88,
  "known_input_tokens": 370538,
  "known_output_tokens": 71984,
  "unknown_usage_attempts": 0,
  "new_estimated_usd": 0.015562596000000001,
  "prior_known_estimated_usd": 0.013749204,
  "cumulative_known_estimated_usd": 0.0293118,
  "authorized_total_usd": 10,
  "paid_search_calls": 0,
  "live_resource_search_calls": 0,
  "first_attempt_utc": "2026-10-11T18:32:17.616353+00:00",
  "last_attempt_utc": "2026-10-11T18:32:31.224966+00:00",
  "routing_wall_max_ms": 773.1977920047939,
  "not_invoice_verification": true
}

Documentation persistence: [SlopSearX PR#782](https://github.com/magnus919/SlopSearX/pull/782). Current merge state is verified separately; no implementation PR.
