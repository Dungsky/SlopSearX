# Baseline drift and intentional differences

F01 inventory baseline: `20109f9083babd014971951e8c02565ae51c2326`.
Original planning baseline: `2a995f964757df77693747b232aa47738c0c58cc`.

## Observed changes

| Baseline interval | Observation | Parity impact |
| --- | --- | --- |
| Planning baseline → F01 baseline | Experiment report and rewrite planning documents merged; no changes in `slopsearx/` or `engines/` | No runtime capability changes; inventory uses the newer full repository revision |
| F01 baseline → `b7b79ab6` (observed while F01 was in review) | Three experiment Markdown documents changed in PR #949 | No source, dependency, workflow, executable or historical Python artifact changes; pinned inventory stays at its declared baseline |

| F01 merged `2e0026b4` → `6aa33a16` (observed during F02) | PR #951 changed 29 experiment Markdown/JSON registration and evidence files | No runtime, dependency, workflow or Python-source changes; architecture and pinned F01 source inventory remain applicable |

| `6aa33a16` → `65a55e2e` | PR #953 changed six experiment Markdown/JSON outcome files | No runtime, dependency, workflow or Python-source changes; F03 begins from F02 merge `562b6ce1` on top of this revision |

## PR #954 drift accepted during F03

Reviewed `562b6ce1` → upstream `73940f07` (PR #954), integrated through a
normal merge into F03. Historical F01 inventory and planning baseline SHAs
remain unchanged. The native F01 fixture pins reviewed current sources; it
checks freshness and does not establish completed native parity.

Two existing mandatory Python tooling modules and four existing test modules
changed, alongside experiment documentation and frozen JSON materials. No
Python files, dependencies, workflow commands or invocation sites were added
or removed. Existing disposition/invocation rows and plan owners remain valid;
no plan dependency or runtime capability changes are required.

| Changed source | Existing owner | Native parity obligation |
| --- | --- | --- |
| `scripts/coverage_band_companion.py` | O03/O08 | Accept exact corrected protocol/cohort/companion pins and v4 groups; reject cross-mixed pins and archival groups |
| `scripts/coverage_gate_calculation.py` | O08 | Accept exact corrected v4 protocol/cohort pins only with unchanged execution and quality/task-use contracts, frozen-pool stage modes and diagnostic-only useful count |
| `tests/test_coverage_band_companion.py` | O05 | Preserve both-stage acceptance, mismatched-pin rejection and group-inventory rejection |
| `tests/test_coverage_gate_calculation.py` | O05 | Preserve both-stage acceptance, unchanged gate/execution identities, mixed/unknown-pin and changed-quality rejection, and hard navigation failure |
| `tests/test_coverage_stage_orchestration.py` | O05 | Exactly representable monotonic test origin avoids rounding at the eight-hour boundary; production deadline semantics remain unchanged |
| `tests/test_coverage_task_use_materials.py` | O05 | Verify only two development queries changed, confirmation and quality contracts remain fixed, and material/candidate source hashes match |

Corrected material hashes: protocol
`cfac29e7d406dc4c7e45030fafeb6ba466923cfeb19b34e427d344507c9d9ba9`, cohorts
`ad731b5d99907242dbc7c6be0c671049e179df76d50a2148886e7e34b6b28b30`, companion
`5b802406687af0a3c1cd318a4f5dfc6250f2643e81547d2739f7521e81740ed2`.
O08 must preserve the exact accepted/rejected cases in successor verification;
O05 carries the new regressions before O09 switches mandatory gates. This
expands parity obligations without authorizing an experiment or changing gates.

## Drift procedure

1. Fetch upstream and follow AGENTS.md session synchronization rules.
2. Compare the last accepted inventory baseline against the new source revision
   with `git diff --name-status OLD..NEW` and inspect changed implementations,
   public declarations, tests, scripts and executable workflow commands.
3. Add or revise capability, Python-disposition and invocation rows; assign a
   plan owner to every addition. Record removed or changed contracts explicitly.
4. Review omitted/null/empty/error semantics and durable bytes/clock behavior.
   Do not normalize a newly observed difference into the replay oracle silently.
5. Update this ledger with the interval, source changes, decisions and affected
   issues. Modify issue scope and dependencies together when needed.
6. Validate exact source-set coverage, owner IDs, source/test references and
   dependency acyclicity before accepting the new baseline. A missing capability
   or unclassified executable blocks acceptance.

F03/F04 will carry this inventory into the native check and replay commands.
Until then, acceptance requires source-set comparison and recorded review;
there is no claim that native parity has been implemented.

## Intentional differences

None implemented or accepted by F01. Proposed hardening in the case study must
receive an explicit ADR, affected contract updates and evidence before it is
counted as an accepted parity difference. Migration of an internal cache
namespace must not implicitly change durable artifact identities.

## F04 temporary developer oracle addition

F04 adds `docs/rust-rewrite/replay/oracle/export.py` against accepted F03
revision `7b2d876307189d7a62000d1f289b3ef4b65eaff6`. This imports the pinned
Python backend to produce reproducible adapter, HTTP and MCP observations
using recorded responses and local fake engines. It is backend-coupled
developer tooling, not a client-only exception. F04 owns its source evidence;
G08 must retire the executable before final backend removal. Its invocation
is recorded separately from mandatory CI/container commands in
`python-invocations.json`. Native replay consumes JSON and never invokes it.

The source manifest grows from 363 to 364 Python paths, with backend-coupled
tooling/test paths growing from 240 to 241. Historical baseline identities
remain unchanged; this ledger records the reviewed post-baseline addition.
The three adapter and two surface observations prove replay infrastructure
coverage only; native implementation parity remains assigned to successor
issues. No live provider capture or experiment is authorized by this addition.

## T01 developer oracle and runtime transport correction

T01 adds `docs/rust-rewrite/replay/oracle/export_mcp.py`, importing the
accepted F04 isolation helpers and observing the clean pinned Python source
at `7b2d876307189d7a62000d1f289b3ef4b65eaff6`. The reference runtime is
Python 3.14.7, FastMCP 4.0.11 and MCP 2.3.0. Injected fixture state and
loopback-only transport observations require no providers or shared store.
This is temporary backend-coupled developer tooling owned by T01, with G08
responsible for executable retirement. Native gates consume captured JSON.
This branch adds one path to the accepted 364-path inventory (365 total;
242 backend-coupled tooling/test). Concurrent F05 additions must be combined
when integrating the branches, not overwritten.

The earlier F01 SSE description inspected the Starlette Mount, which has
no method list, and incorrectly inferred that its handler had no explicit
method guard. T01 actual runtime captures show HTTP 405 for non-POST
message requests and request-body size enforcement. The surface inventory
now reflects the handler behavior. This is an evidence correction, not an
intentional compatibility change; original baseline evidence remains in Git.
