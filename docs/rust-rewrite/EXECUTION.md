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
  GPT-6.1 Sol (`gpt-6.1-sol`), per the maintainer update on 2026-10-15.
  Earlier F01/F02 assignments used GPT-6 Sol; new complex assignments use 6.1.
- Use isolated task worktrees. Within a task, parallel agents own disjoint files;
  the orchestrator owns integration and shared contracts.

An issue starts only after its hard prerequisites have accepted and merged
handoffs. Open status alone does not mean ready. No issue closes merely because
an agent reports completion. Inspect actual source, run relevant checks, inspect
review findings and exact-head CI, and verify the merged artifact before closure.

## Execution ledger

| Plan | Issue | State | Evidence / next gate |
| --- | --- | --- | --- |
| F01 | [#880](https://github.com/magnus919/SlopSearX/issues/880) | Accepted | [PR #950](https://github.com/magnus919/SlopSearX/pull/950), merged `2e0026b4e471c9309a898665169578333074015e`; [inventory evidence](inventory/validation.json), exact-head CI passed, independent Sol corrections and Luna final audit accepted |
| F02 | [#881](https://github.com/magnus919/SlopSearX/issues/881) | Accepted | [PR #952](https://github.com/magnus919/SlopSearX/pull/952), merged `562b6ce1ce9f798104311e8f3a213daefeddc795`; ADR 003–005 and state/pin registers; corrected membership classification, positive Droid re-review and exact-head CI passed |
| F03 | [#882](https://github.com/magnus919/SlopSearX/issues/882) | Accepted | [PR #955](https://github.com/magnus919/SlopSearX/pull/955), merged `7b2d876307189d7a62000d1f289b3ef4b65eaff6`; exact-head CI and completed Droid re-review; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/882#issuecomment-6200997449), retained Linux amd64/arm64 native evidence |
| F04 | [#883](https://github.com/magnus919/SlopSearX/issues/883) | Accepted | [PR #957](https://github.com/magnus919/SlopSearX/pull/957), merged `16f03be656f34d90edd8285f9c81dfb1c4626b0a`; exact-head CI, independent checks and final review passed; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/883#issuecomment-6202578924); retained Linux amd64/arm64 replay evidence |
| F05 | [#884](https://github.com/magnus919/SlopSearX/issues/884) | Accepted | [PR #960](https://github.com/magnus919/SlopSearX/pull/960), merged `7cc420f7ffb61127750e77bf6130e225925f9186`; typed domain and stable serialization evidence; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/884#issuecomment-6203899560) |
| F06 | [#885](https://github.com/magnus919/SlopSearX/issues/885) | In progress | Dispatched after accepted F05 handoff; isolated implementation and acceptance pending |
| F07 | [#886](https://github.com/magnus919/SlopSearX/issues/886) | Accepted | [PR #961](https://github.com/magnus919/SlopSearX/pull/961), merged `cfd7a0af61a22cfd040a7fcd41ad950f82005959`; native Valkey lifecycle and exact atomic-script evidence; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/886#issuecomment-6204292044), [scope and parity ledger](f07/README.md) |
| F08 | [#887](https://github.com/magnus919/SlopSearX/issues/887) | Accepted | [PR #962](https://github.com/magnus919/SlopSearX/pull/962), merged `9ab1c7a22de933762c42469145c1f50365220de2`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/887#issuecomment-6205234733), independent source review and exact-head Linux amd64/arm64 CI passed |
| F10 | [#889](https://github.com/magnus919/SlopSearX/issues/889) | Accepted | [PR #966](https://github.com/magnus919/SlopSearX/pull/966), merged `c326a3d5a75fd299b05ebe366eac816352b3d36d`; native quota authority, provider cooldown and observed-health evidence; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/889#issuecomment-6206445353), [scope/evidence](f10/README.md) |
| F11 | [#890](https://github.com/magnus919/SlopSearX/issues/890) | Accepted | [PR #963](https://github.com/magnus919/SlopSearX/pull/963), merged `fd80e36e6e1868d5c47f3f3d93c78952fcbc20e5`; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/890#issuecomment-6204838740), independently recaptured source corpus and exact-head Linux amd64/arm64 CI passed |
| T01 | [#895](https://github.com/magnus919/SlopSearX/issues/895) | Accepted | [PR #959](https://github.com/magnus919/SlopSearX/pull/959), merged `5058056b02c5ebe523e61acfaadf19795f664be0`; native MCP SDK, transport and lifecycle qualification; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/895#issuecomment-6206106156) |
| W01 | [#905](https://github.com/magnus919/SlopSearX/issues/905) | In review | Durable research records, fenced private-Valkey mutations and conserved accounting; accepted F11 lead identity integrated; [scope/evidence](w01/README.md), independent identity review and exact-head CI pending |
| O03 | [#920](https://github.com/magnus919/SlopSearX/issues/920) | Accepted | [PR #965](https://github.com/magnus919/SlopSearX/pull/965), merged `5395518939cf4f48ee9063e950b8ad4363503143`; active experiment tooling inventory and prospective native evidence contracts; [accepted handoff](https://github.com/magnus919/SlopSearX/issues/920#issuecomment-6206715399); no experiment or provider execution |
| O06 | [#935](https://github.com/magnus919/SlopSearX/issues/935) | Accepted | [PR #958](https://github.com/magnus919/SlopSearX/pull/958), merged `f539c0c2f597cd2fcfc382d343d08465baf11283`; exact-head CI and independent/Droid reviews passed; O09 owns gate activation |

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
