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
