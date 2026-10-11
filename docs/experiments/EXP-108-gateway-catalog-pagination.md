# EXP-108: preserve paginated remote tool discovery

## Frozen registration

- Additional cycle explicitly requested by maintainer on 2026-10-11; start 2026-10-11T13:03:00Z. Default 45-minute ceiling; zero provider calls/cost.
- Issue #778. Consequential problem: agents connected through the gateway cannot discover or invoke tools on later upstream discovery pages.
- Evidence class: protocol defect correction, authenticated MCP HTTP upstream → real gateway lifespan/HTTP → native client. Fixture tools return their identities; no retrieval or agent-task benefit claim.
- Baseline: 6aedb033e607d3e0e9d6ef4e6984371dad1f8796, root Python3.12/FastMCP3.4.7/MCP1.29.0 environment. Readiness baseline: 0/2 later-page tools discoverable and callable; first-page control passes. Earlier fixture setup failures retained, not scientific trials.
- Candidate: follow opaque cursors across all catalog pages before proxy registration. Bound to 128 pages/4096 tools and existing connect_timeout wall time; reject repeated cursors and duplicate names rather than publish an incomplete/ambiguous catalog.
- Primary: count of later-page tools both discovered and returning exact expected identity through gateway. Minimum useful effect: 0/2 → 2/2 (+100pp); all tools in this exhaustive three-page fixture must work.
- Guardrails: first-page call unchanged; no auth/policy widening; single-page and empty catalogs unchanged; snake/camel SDK cursor aliases accepted; empty-string cursors followed; repeated cursor, duplicate name, over-limit catalog and later-page errors reject startup before registration; failed discovery leaves no live session state. No result or resource schema change.
- Sample/stopping: frozen worker SHA below; one baseline-ready acquisition already retained, one candidate replay. No exclusions/tuning or primary retry. Deterministic finite corpus, uncertainty confined to cases; production prevalence unknown. Full required checks/review before merging implementation. Retain shutdown diagnostics if present, not claim lifecycle benefit.
- Reproduction: `PYTHONPATH=. .venv/bin/python docs/experiments/evidence/EXP-108/worker.py.txt /private/tmp/exp108-new-output`. Both disposable servers bind loopback; static fixture auth only, no engine dispatch.
- Portal: gateway-only discovery; shared service/HTTP portal unaffected. Existing portal CI gate still required.
- Supported only if primary and all guardrails pass. Preserve other outcomes and discard code only after retaining evidence. No deploy or UI-host support claim.

Worker SHA-256: `60ea4dae35496a36b80361ed79ba0ce20f64fbb524342c0882cd32dd5d4074d7`.

## Administrative timestamp correction

The start copied into registration was incorrect. The authoritative app thread
startedAt is 2026-10-11T16:36:58Z; see the retained correction receipt. The
45-minute ceiling runs from that actual start (deadline 17:21:58Z). Baseline,
worker, sample, gates and stopping criteria were unchanged.

## Readout — supported protocol correction

Registration `c140c76`; baseline `6aedb03`; final candidate
`07bb27b4fb23ecce06be8aab18635dc7ab5665a1`. Frozen primary **0/2 → 2/2**
later-page tools both discoverable and callable (+100 percentage points).
First-page identity control unchanged. Both arms executed the same frozen
worker once after fixture readiness; no primary exclusion, tuning or retry.

Guards cover opaque/empty cursors, snake/camel SDK aliases, empty/single-page
catalogs, duplicate names, repeated cursors, page/tool limits, later-page errors
and discovery timeout/session cleanup. Actual discovery uses the same
authenticated remote session and registers only a complete bounded catalog.
No shared search service, cache, policy, result schema or portal UI change.

Local validation: 134 gateway/API/formatter tests, 21 final gateway tests, three
portal browser tests, mypy (121 files) and changed-file hooks passed. All-file
hooks formatted unrelated research paths; retained patch and restored paths.
AST-only graph updated. Initial wrong test-path invocation retained. Local full
suite failed its locked-environment test (also failed unchanged baseline), then
timed out in MCP harness; neither is counted passing. Both replay workers
completed with exit zero but emitted the same cancel-scope shutdown diagnostic;
no lifecycle improvement claim.

First CI failed because the newer native MCP SDK requires actual streams in the
probe. A test-only correction supplied in-memory streams and reran affected
local checks; implementation/primary/gates unchanged. The failed CI is retained.
Droid subsequently rejected its stale checkout; automated harness review was
incomplete. A substantive runner review of the exact corrected SHA found no
actionable issues, recorded on the PR before merge.

This establishes correction of missing later-page tool capabilities in the
fixed protocol cases. Production frequency, retrieval quality, agent-task uplift
and ChatGPT UI host support are unmeasured. Issue #487 remains broader. No
deployment, alerting or SLO operations. Experiment provider calls/cost zero;
automatic CI reviewer request/token observations retained separately, cost and
subscription usage unknown.

Delivery: [#779](https://github.com/magnus919/SlopSearX/pull/779), author
magnus919, verified merged at 2026-10-11T16:55:14Z as
`0f6aa187bdfa4220d9875f49ee0ae2e2a741d0e2`. All applicable checks on
`07bb27b` passed, including both full Python suites, portal, Valkey, API,
quality/security and builds. Exact author/SHA/CI and the substantive runner
review were verified before merging without protection bypass.
[Evidence](evidence/EXP-108/README.md) retains all outcomes, reproduction,
validation logs, failed CI, final CI summary, review and verified merge.
Release/production status unverified. The earlier readiness-only cycle was
followed by this additional run at the maintainer's explicit request.

Protocol basis: [MCP tools discovery](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) supports cursor pagination; the bounded replay tests preservation of that contract.
