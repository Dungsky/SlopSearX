# Ten-paper bounded experiment campaign — 2026-10-11

This campaign is explicitly authorized by the maintainer, with Codex-authored planning pilots and a $10 total paid-call ceiling. It is a sequential research campaign, not a background scheduler. Source review is preserved in [research](evidence/ARXIV-CAMPAIGN-2026-10-11/research.md).

## Sequence and interaction controls

1. EXP-118 evaluation readiness establishes external labels and paired service replay.
2. EXP-113 fusion first: inexpensive, direct seam. k20 is a fixed challenger, not assumed better. Keep shipped baseline for every single-component comparison even if it wins.
3. EXP-109 rewriting tests retrieval coverage; EXP-110 interleaving requires multi-hop requirements and source-attributed evidence.
4. EXP-111 stopping and EXP-112 resource selection reduce acquisition work and can conflict with recall gains from rewriting. Test each alone before combination.
5. EXP-114 selective reranking and EXP-115 candidate depth cut compute on captured pools; they can compound losses by bypassing/dropping relevant candidates.
6. EXP-117 diversity tests coverage/relevance tradeoffs; it can undo useful navigation ranking or reranker improvements.
7. EXP-116 freshness needs temporal data; it can save calls but obscure planner/ranker effects. Quality experiments remain cache-disabled.

Large potential impact: query coverage and evidence-dependent planning; resource selection and stopping have systemic cost/recall consequences. Direct low-cost priority: fusion. Reranking efficiency matters only if reranking adds value; benchmark reranking is explicitly qualified before recommending efficiency. Evaluation is prerequisite, not an improvement claim. Freshness is independent and higher-risk.

## Compound experiments (conditional, not authorized by a winning headline)

No selected combination may reuse its selection cohort as confirmation. After individual confirmation, freeze a separate four-arm factorial study on fresh query families: incumbent, A, B, A+B. Interaction = (A+B−B)−(A−incumbent). Require useful combined primary effect, unchanged hard guards and no worse domain losses. Adjust for the number of actually registered combinations. Suggested compatible pairs: rewriting + fusion; planning + stopping; routing + stopping; selective reranking + depth. Test diversity after ranking, with facet labels before facet claims. Keep cache studies separate with cache-disabled controls and real temporal workload.

Budget and sequencing never justify tuning on confirmation, relabeling failures, running #516 again, or claiming missing evidence proves an idea false. Earlier planning pilots and shipped caller-directed tools are prior context, not new confirmation or new feature credit.

## Candidate registry

| Experiment | Paper | Primary | State |
|---|---|---|---|
| [EXP-109](EXP-109-query-rewriting.md) | 1 | nDCG@10 gain >=0.05 | planned |
| [EXP-110](EXP-110-evidence-conditioned-planning.md) | 2 | required evidence recovery gain >=0.15 | planned |
| [EXP-111](EXP-111-adaptive-query-stopping.md) | 3 | request saving >=20% | planned |
| [EXP-112](EXP-112-resource-selection.md) | 4 | resource request saving >=25% | planned |
| [EXP-113](EXP-113-fusion-calibration.md) | 5 | nDCG@10 gain >=0.05 | planned |
| [EXP-114](EXP-114-selective-reranking.md) | 6 | scorer input-token saving >=25% | planned |
| [EXP-115](EXP-115-adaptive-reranking-depth.md) | 7 | scorer input-token saving >=25% | planned |
| [EXP-116](EXP-116-freshness-cache.md) | 8 | stale-serving error reduction >=0.05 | planned |
| [EXP-117](EXP-117-result-diversity.md) | 9 | recall@10 gain >=0.05 | planned |
| [EXP-118](EXP-118-cross-domain-evaluation.md) | 10 | measurement readiness; no user-benefit primary | planned |

## Decision/preservation rules

Register activation before each candidate; preserve raw inputs, external qrels hashes, command receipts, provider costs/errors and outcome. Supported confirmation yields one focused feature-request issue specifying before/after, evidence, limits, implementation seam, interactions and acceptance/rollback. Negative, inconclusive, blocked and readiness records remain documented. No auto-implementation.

## Maintainer ownership clarification — before execution

SlopSearX remains confined to meta-search. EXP-109/110/111 are routed experiment requests, not local candidate runs. Answer/page-cache reuse and generated research facets are also wrapper-owned; core EXP-116 stays search-response-only and EXP-117 stays deterministic result selection.

- [experiment: evaluate caller query rewriting before SlopSearX meta-search](https://github.com/magnus919/groktocrawl-x/issues/444)
- [experiment: evidence-conditioned research follow-ups with source-grounded dependencies](https://github.com/magnus919/groktocrawl-x/issues/445)
- [experiment: adaptive continue/stop decisions for bounded research](https://github.com/magnus919/groktocrawl-x/issues/446)
- [experiment: risk-gated answer and page cache reuse in research workflows](https://github.com/magnus919/groktocrawl-x/issues/447)
- [experiment: generated evidence facets for diverse research source selection](https://github.com/magnus919/groktocrawl-x/issues/448)

EXP-118,113,112,114,115,117 registrations activated prospectively under their signed fixed plans; execution remains sequential. Dataset query metadata containing original support labels is excluded from candidate inputs; only text fields are projected. Confirmation stays unopened until its gate.

## Current disposition

[Readout and evidence](evidence/ARXIV-CAMPAIGN-2026-10-11/readout.md): five completed meta-search development screens, no advancement/feature request, evaluation readiness qualified, temporal freshness blocked, wrapper-owned experiments requested in#444–448. No confirmation/compound execution or runtime/default/deployment change.

| Experiment | Final disposition |
|---|---|
| EXP-109 | routed #444 |
| EXP-110 | routed #445 |
| EXP-111 | routed #446 |
| EXP-112 | inconclusive |
| EXP-113 | not-supported |
| EXP-114 | not-supported |
| EXP-115 | not-supported |
| EXP-116 | blocked; temporal data |
| EXP-117 | not-supported |
| EXP-118 | measurement readiness qualified |
