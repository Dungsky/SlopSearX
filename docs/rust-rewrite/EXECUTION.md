# Rust Rewrite execution and acceptance

The maintainer authorized execution of the complete Rust Rewrite milestone on
2026-10-15. The target remains the full [case-study contract](CASE_STUDY.md),
including native backend, tooling, gateway and ssx, operational qualification,
cutover, retirement and final acceptance. No intermediate foundation is a
substitute for milestone completion.

## Execution ownership

- Orchestrator: parent agent, accountable for integration, review, acceptance,
  issue evidence, dependency readiness and final completion audit.
- General execution: GPT-6 Luna (`gpt-6-luna`).
- Particularly complex architecture, identity and distributed-state work:
  GPT-6 Sol (`gpt-6-sol`).
- Use isolated task worktrees. Within a task, parallel agents own disjoint files;
  the orchestrator owns integration and shared contracts.

An issue starts only after its hard prerequisites have accepted and merged
handoffs. Open status alone does not mean ready. No issue closes merely because
an agent reports completion. Inspect actual source, run relevant checks, inspect
review findings and exact-head CI, and verify the merged artifact before closure.

## Execution ledger

| Plan | Issue | State | Evidence / next gate |
| --- | --- | --- | --- |
| F01 | [#880](https://github.com/magnus919/SlopSearX/issues/880) | In progress | Baseline `20109f9083babd014971951e8c02565ae51c2326`; inventory manifests undergoing source-based reconciliation |

All other packages remain pending. [The dependency index](ISSUES.md) and native
GitHub dependencies determine readiness. Update this ledger with merged PR and
handoff evidence as packages are accepted; retain failed checks and corrections.

## Completion audit

At G09, rederive requirements from current issues, accepted ADRs, capability and
Python-disposition manifests, the case study and maintainer instructions. For
each requirement, record direct evidence or mark it incomplete. Verify all
public surfaces, adapters, state/recovery behavior, browser journeys, native
clients/tooling, exact release artifacts and deployed processes/schedules.
Tests and static manifests only prove the cases they actually cover. Close the
epic and milestone only when every requirement has sufficient current evidence.
