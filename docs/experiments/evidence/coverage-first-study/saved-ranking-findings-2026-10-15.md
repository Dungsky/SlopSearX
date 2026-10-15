# Saved research-ranking findings — 15 October 2026

The tested candidate is not qualified for production. Jev completed all 39 ranking calls, but a test-runner bookkeeping defect stopped the study before answers were generated. Separate offline analysis found promising usefulness scores and a concrete evidence-coverage problem. No confirmation run followed.

## What completed

The development stage `e13bf092-e63a-45bb-beda-8fc4a5d13965` used source `0974b19dbadf80b07f9be9bb20fec1de5ada4b47`. All 53 reference-grading submissions completed and their closure check passed. All 39 serial Jev operations succeeded. Their recorded total usage was 824,816 input tokens and 97,331 output tokens. The slowest recorded whole ranking phase was 467 ms, below the registered 1,000 ms limit.

The runner copied a ranking outcome such as `selected` or `fallback` into a field that required execution state `complete-success`. Its next validation therefore rejected the results. This is a runner defect, not 39 failed Jev requests. The original stage remains stopped and incomplete; correcting the defect cannot retroactively turn it into a passed study.

There were no answer calls, answer assessments, confirmation calls, retries, or new search/capture calls. Development and unused confirmation identities are retired. Deployments were unchanged.

## What the saved rankings show

Two offline analyses used the saved responses and completed independent reference assessments. Neither made model or search calls. Both included all eight base research cases and used the registered nDCG@10 calculation, 10,000 bootstrap samples, seed 7701, and unchanged quality thresholds. W0 is the matched query-only incumbent order from the saved task manifest; this analysis does not establish a separate live native-order baseline.

| Policy | Mean nDCG@10 improvement over W0, assessor A / B | Important limitation |
| --- | --- | --- |
| Original candidate, including its fallbacks | +0.0447 / +0.0416 | Seven of eight cases kept W0 unchanged; both confidence intervals include zero |
| Sort by the existing usefulness scores | +0.4348 / +0.4266 | One case lost both required useful evidence facets |

For the original candidate, the bootstrap intervals were [0, 0.1340] and [0, 0.1247]. The one changed case improved, but the positive-lower-bound requirement did not pass. No per-case useful-result count or useful-facet loss was observed.

The single fixed Score-only analysis sorted the original reported scores descending with the existing canonical tie-break, without changing scores, questions, facets, or thresholds. Its intervals were [0.3362, 0.5593] and [0.3325, 0.5551]. Seven cases passed the per-case checks for both assessors. In R05, however, the top results lost both required useful facets, despite an increase in aggregate ranking quality. That fails the coverage requirement.

Across all 17 candidate operations, 16 fell back because a lead was unavailable for at least one required facet; only one selected a new order. The all-or-nothing safeguard usually discarded the useful scoring signal. Removing it entirely produced the R05 evidence loss.

These are exploratory findings from exposed development data. They do not establish completed research-task gains, untouched confirmation, navigation, stability, or every finalization requirement. They cannot rescue the stopped primary study.

## Delivery decision

Keep the production ranking unchanged. Do not ship either tested policy or start confirmation. The remaining product question is specific: can a ranking preserve necessary evidence while retaining the usefulness gains in the existing scores? Any revised policy must be specified before its evaluation and must preserve the failed results above. A development-only improvement is insufficient for release.

Issue #516 still requires a supported policy, complete development and independent confirmation, then shared-service implementation and compatibility verification across API, MCP, portal, cache, and GroktoCrawl forwarding. There should be one shared reranker. No production deployment follows from this record.

## Evidence identity

- Protocol SHA-256: `65b5515eb161c849c54f196c300f03b3965cbdbd4162682131b13ccecd0911a8`.
- Task-input manifest SHA-256: `d23b73091aabc0cba9981d5f04fb763fefd0f83f12e21842450d5bdcc257d002`.
- Closed reference outputs SHA-256: `d949f4c1f971d8c01b861a54e12bfbb211dd03fa4a1d1362712ca530afe64702`.
- Final stage inventory SHA-256: `b1d914293740fe29914cb25f3f9793eaca85e9eb4e7ef662642085c274c32c2b`.
- Closeout receipt SHA-256: `74757486ebd83a5fef646c0334a85caac418074713f89f3b83cc3649bf703ef5`.

The sealed source artifacts, reproducible offline analysis scripts, and detailed readouts are retained privately. This public summary excludes operational paths, credentials, and deployment identifiers.
