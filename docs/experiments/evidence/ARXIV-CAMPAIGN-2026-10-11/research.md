# Ten arXiv papers for future SlopSearX experiments

Access date: 2026-10-11. Checkout: a39f795129313d3355d8ed1a0e4d6bcc7ed33ec6.

## Brief and evidence boundary

Question: which research mechanisms offer testable improvements in retrieval usefulness or end-to-end latency across different SlopSearX components? This is a targeted primary-source literature scan, not a systematic review or benchmark reproduction. Ten is the requested shortlist size. All reported paper findings are author claims; every SlopSearX adaptation is a proposal. No experiments or implementation changes were made.

Durable destination: this repository document. Source-level records below preserve provenance, method, evidence, limitations and experiment relationships separately from the synthesis. The pre-existing untracked October 6 research note was read and preserved unchanged.

## Verified project fit

The graph report was inspected for central abstractions: EngineStatus, SearchResult, EngineAdapter, CapabilityCatalog and SearchService. Its September 21 graph and inferred edges guide navigation rather than prove current behavior. No graph wiki index was found.

Current sources inspected: research.py:167 has fixed intent/suffix plans; docs/ADAPTIVE_RESEARCH.md documents caller-directed continuation. jev.py:207 supplies existing specialist selection. merger.py:18/86 provides presence and optional k=60 RRF within mandatory tiers. Raw upstream scores are incomparable. cache.py:98 already shortens news and partial-result lifetimes. service.py:884 dispatches concurrent tasks under concurrency limits and timeouts, with an optional interactive deadline.

The experiment ledger supplies important counterevidence: science fixtures regress under RRF; EXP-004 could not evaluate reranking because canonical targets were absent; EXP-023 routing composition failed its advance gate. Ranking cannot recover missing candidates, and routing classification is not evidence of useful retrieval.

## Source records and experiment candidates

### 1. Query Rewriting for Retrieval-Augmented Large Language Models

Xinbei Ma et al., 2023. [arXiv:2305.14283](https://arxiv.org/abs/2305.14283). Primary abstract inspected.

Method/evidence: Rewrite-Retrieve-Read adapts the search query; a small rewriter can learn from reader feedback. Authors report improvements on open-domain and multiple-choice QA.

Proposal: caller-generated concise rewrites through existing research dispatch. Compare original versus rewritten queries at equal adapter-call budgets, then original-plus-rewrite against two baseline variants. Measure useful evidence, top-ten relevance, query drift and total latency/cost. Preserve exact entities, dates, versions and identifiers.

Limitation: QA gains do not establish search-snippet quality across upstream APIs. Start with prompting before expensive RL. Priority: high.

### 2. Interleaving Retrieval with Chain-of-Thought Reasoning for Knowledge-Intensive Multi-Step Questions

Harsh Trivedi et al., 2022 preprint / ACL 2023. [arXiv:2212.10509](https://arxiv.org/abs/2212.10509). Primary abstract inspected.

Method/evidence: IRCoT alternates retrieval and reasoning because later information needs depend on earlier discoveries; evaluated on four multi-hop datasets including HotpotQA and MuSiQue.

Proposal: source-attributed, evidence-conditioned follow-ups using research continuation. Compare upfront decomposition with follow-ups at equal budgets. Measure dependent-subquestion coverage, unsupported entity propagation, duplicate searches and time to sufficient evidence. Use concise evidence requirements rather than expose private reasoning traces.

Limitation: controlled QA corpora differ from live search, and intermediate assertions require verification. Priority: high.

### 3. When should I search more: Adaptive Complex Query Optimization with Reinforcement Learning

Wei Wen et al., January 2026. [arXiv:2601.21208](https://arxiv.org/abs/2601.21208). Primary abstract inspected; HTML fetch failed.

Method/evidence: ACQO combines adaptive reformulation/decomposition, rank-score fusion and curriculum RL; authors report results on three complex-query benchmarks.

Proposal: test whether another query adds enough evidence to justify its cost. Compare a bounded continue/stop rule with fixed query counts; label each attempt's marginal evidence. Measure useful coverage, premature stops, calls and p95 elapsed time.

Limitation: a simple caller-side stopping rule is a related adaptation, not reproduction of trained ACQO. Full-method review is needed before registration; abstract-only confidence. Priority: medium, after candidate 2.

### 4. ReSLLM: Large Language Models are Strong Resource Selectors for Federated Search

Shuai Wang et al., 2024. [arXiv:2401.17645](https://arxiv.org/abs/2401.17645). Primary abstract inspected. Earlier October 6 project note contains additional analysis, treated as historical project context.

Method/evidence: zero-shot selection from resource descriptions; Synthetic Label Augmentation Tuning trains selection from model-judged query/snippet logs. Authors report competitive selection and stronger tuned performance.

Proposal: improve existing Jev routing with independently audited marginal retrieval-value labels. Hold out query families. Measure useful evidence in the top ten, essential-source misses, unnecessary calls and routing-plus-search latency.

Limitation: synthetic labels and selection accuracy may not predict real evidence gains; existing project routing experiments caution against this inference. Keep live eligibility and policy authoritative. Priority: medium; requires an audited corpus.

### 5. An Analysis of Fusion Functions for Hybrid Retrieval

Sebastian Bruch, Siyu Gai and Amir Ingber, 2022 preprint. [arXiv:2210.11934](https://arxiv.org/abs/2210.11934). Primary abstract inspected.

Method/evidence: compares convex score combination and reciprocal-rank fusion; finds RRF parameter-sensitive and score combination sample-efficient in its hybrid-retrieval setting.

Proposal: on fixed captured pools compare presence, shipped k=60 RRF, held-out-tuned k and engine-weighted rank fusion. Preserve tiers and actual specialist promotion. Measure domain-specific nDCG@10, first useful result and CPU time.

Limitation: paper concerns lexical/dense fusion, not many upstream engines. Do not average uncalibrated raw API scores. Weighted rank fusion is our adaptation. Existing science regressions must remain visible. Priority: high; low implementation burden.

### 6. Adaptive Re-Ranking

Ata Cinar Genc, Emir Kaan Korukluoglu and James Allan, June 2026. [arXiv:2606.25249](https://arxiv.org/abs/2606.25249); [full text](https://arxiv.org/html/2606.25249v1). Abstract and result passages inspected.

Method/evidence: utility-based per-query routing among BM25, MiniLM and BGE paths. Reported mean latency reductions of 1.11–5.22x accompany nDCG@10 changes from −17.5% to +4.0%.

Proposal: if reranking is useful, compare current, always-rerank and selectively-rerank paths on the same cards. Include router overhead, quality by domain, mistaken bypasses, compute and p50/p95 total latency.

Limitation: substantial quality losses occur in the paper itself. SlopSearX does not have this exact model pipeline; snippets may be insufficient. Priority: medium, conditional on reranking value.

### 7. Ranked List Truncation for Large Language Model-based Re-Ranking

Chuan Meng et al., 2024. [arXiv:2404.18185](https://arxiv.org/abs/2404.18185). Primary abstract inspected; attempted HTML v2 yielded no usable text.

Method/evidence: examines eight truncation methods across three retrievers and two rerankers on TREC DL 2019/2020; candidate depth is query-dependent.

Proposal: fixed versus adaptive numbers of cards sent to a reranker. Measure relevant candidates excluded, final nDCG@10, tokens and latency. This asks how much to rerank, separately from candidate 6's decision whether to rerank.

Limitation: trim optional reranker input only. Preserve the full canonical response and snapshot; max_results is a presentation bound. Priority: medium, conditional on reranking value.

### 8. Risk-Constrained Freshness-Aware Semantic Caching for Open-Web Retrieval-Augmented LLMs

Muhammad Mansoor, Tahir Ahmad and Yeo-Chan Yoon, July 2026. [arXiv:2607.04281](https://arxiv.org/abs/2607.04281); [full text](https://arxiv.org/html/2607.04281v1). Abstract, methods, limitations and external-validation passages inspected.

Method/evidence: FreshCache gates reuse by estimated staleness risk. Strong headline savings depend on its benchmark. External EvolvingQA evaluation reports 29.2% cache-induced stale error among reused queries; DailyQA transfer is weaker. Paraphrase-heavy simulations and precomputed similarity limit latency conclusions.

Proposal: first test freshness-aware exact-key reuse against existing category/partial TTL rules. Use timestamped replay and independently checked answer-changing evidence. Measure stale-serving rate, useful reuse, avoided calls and actual p95 latency, including news, jobs, versions and security updates.

Limitation: page change differs from answer change; freshness prediction is not a guarantee. Semantic equivalence is a separate later hypothesis. Retain canonical keys, scope/filters, Valkey and caller freshness semantics. Priority: exploratory, higher risk.

### 9. IntenT5: Search Result Diversification using Causal Language Models

Sean MacAvaney et al., 2021. [arXiv:2108.04026](https://arxiv.org/abs/2108.04026). Primary abstract inspected.

Method/evidence: generates query intents without massive proprietary logs; tested on six diversification benchmarks, particularly useful for multifaceted queries.

Proposal: diversify frozen results by evidence facet within tiers. Compare relevance-only ranking and a simple redundancy penalty before training a model. Measure facet coverage, alpha-nDCG@10, ordinary relevance and processing overhead.

Limitation: generated facets can alter user intent. Separate multifaceted, ambiguous and exact-identifier queries. The deterministic baseline is an adaptation, not IntenT5 reproduction. Priority: medium-high.

### 10. BEIR: A Heterogenous Benchmark for Zero-shot Evaluation of Information Retrieval Models

Nandan Thakur et al., 2021. [arXiv:2104.08663](https://arxiv.org/abs/2104.08663). Primary abstract inspected.

Method/evidence: 18 datasets span tasks/domains; strong average reranking comes with computational cost, and narrow tests hide generalization failures.

Proposal: selected BEIR tasks for offline mechanism checks plus a separately judged captured SlopSearX suite spanning web, code, science, jobs and freshness. Hold out query families; report every domain and the latency/quality frontier.

Limitation: closed-corpus scores do not prove live availability, portal usefulness or agent task completion. This improves experiment decisions rather than directly accelerating requests. Priority: foundational.

## Synthesis and proposed order

Build the diverse evaluation backbone alongside fusion calibration (5) and evidence-conditioned continuation (2). Then rewriting (1) and diversity (9). Routing (4) needs marginal-value labels; stopping (3) needs coverage labels. Selective reranking/depth (6/7) should follow proof of useful reranking. Freshness (8) requires a temporal corpus before serving changes.

For quality-first trials, preregister a minimum useful relevance improvement and bounded latency increase. For speed-first trials, preregister p95 savings and a quality noninferiority margin. Set numerical gates from baseline variance before candidate work. Use paired query-level uncertainty and family-held-out tuning; count queries, not documents, as independent observations. Include equal-call-budget controls, planning overhead, cache misses, errors and tails. More URLs, classification F1, cache hits or isolated model timings are insufficient primary outcomes.

All future dispatch retains shared policy/filter gates; all changes retain canonical cache/snapshot integrity and mandatory tiers. Shared state remains in Valkey. Browser-visible ranking/scope/cache changes require portal impact review and coverage. This document changes no browser contract.

## Research log and exclusion ledger

Searches used arXiv-targeted web queries across rewriting, adaptive retrieval, resource selection, fusion, semantic caching, hedged-request/tail latency, diversity, candidate-depth and evaluation. Direct title lookups verified IRCoT and RankGPT. Primary arXiv records were used for evidence; secondary aggregators were excluded. Memory located historical cache/gateway work only; current checkout governed project fit.

All ten retained sources appear in the records above, including inspected depth, methods, material claims, limitations and proposed tests. No benchmark findings were reproduced. Before a faithful reproduction, inspect complete methods and code/artifacts and register a separate experiment.

Considered exclusions:

- Query2doc (2303.07678), RankGPT (2304.09542), Decomposed Prompting (2210.02406): already developed in the October 6 note. Candidates 1/2 retain continuity while 3/6/7 add different decisions.
- Dynamic Ranked List Truncation (2604.09492): overlaps 7; broader comparative study selected first.
- MA4DIV (2403.17421): heavier MARL training; IntenT5 fits limited query logs better.
- CacheRAG (2604.26176): knowledge-graph/schema focus weaker for present open-web cache.
- Grounded Cache Routing (2605.27494): answer-cache safety may be useful later; search freshness selected here.
- When Do Redundant Requests Reduce Latency? (1311.2851): equivalent replicas differ from non-equivalent search engines; duplicate paid requests are not justified by this analogy.
- Size-aware Sharding (1802.00696), Taming Tail Latency (1703.08337), Deconstructing the Tail at Scale Effect (1701.03100): storage/network internals offer weaker direct fit; no reason to replace Valkey from these findings.
- BRIGHT (2407.12883): relevant reasoning-heavy extension; BEIR provides broader evaluation coverage for this shortlist.

Confidence: moderate for mechanism relevance; unknown for SlopSearX uplift. Most papers received abstract-level screening; 6/8 received additional full-text result/limitation inspection. No exhaustive/latest-version or peer-review-status claim.

Open questions: upstream query-parser compatibility; snippet sufficiency; cross-domain transfer; routing overhead; actual recurrence/volatility; human and agent task benefit.

Preservation inventory: this durable report contains brief, source records, synthesis, search decisions, exclusions and open questions. Every retained source and material finding is accounted for; failed HTML access is recorded; no essential evidence is left only in chat. The earlier note remains unchanged. This is an experiment-inspiration deliverable, not an executed experiment.
