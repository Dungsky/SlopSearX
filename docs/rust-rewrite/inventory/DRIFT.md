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

## F05 temporary pure-function oracle addition

F05 adds `docs/rust-rewrite/f05/oracle/export.py` against accepted F04
revision `16f03be656f34d90edd8285f9c81dfb1c4626b0a`. The exporter observes
pinned Python storage codecs, HTTP/MCP projections and canonical identities
using local inputs. It is backend-coupled developer tooling; it does not
qualify for the client-only exception. F05 owns capture provenance and G08
owns executable retirement. Native F05 checks consume portable JSON without
launching Python. No provider calls or deployment are authorized.

This branch adds one path to the accepted 364-path manifest: 365 total,
including 242 backend-coupled tooling/test paths. Historical F01 baseline
reports remain unchanged. The reference environment uses Python 3.14.7 and
Unicode 16.0.0 to match the production image's Python version; the Unicode
identity cases include mappings absent from Python 3.12's Unicode 15 tables.

## F05 explicit native input differences

The F05 implementation introduces the following differences for review with
ADR 003; final acceptance requires the F05 negative tests and reviewed PR.
Pure JSON parsing rejects input larger than 4 MiB or nested more than 100
levels, and rejects floating-point tokens that cannot be represented as a
finite f64. Arbitrary-width integer digits remain lossless. These are explicit
native input bounds, not a claim that every legacy record fits. Payload and
storage codecs apply their narrower record limits, including source-compatible
omission where applicable. Persisted-data compatibility still requires the
later store and migration qualification; these parser tests do not prove it.

Native artifact-reference decoding requires integer JSON version `1` and
rejects boolean `true`. The Python equality check accepts `true`, although
its producers write integer `1`. Negative tests must cover this rejection
and each parser bound. Existing opaque identifiers remain unchanged; this
change migrates no durable identity namespace. Public numeric values retain
integer-versus-float representation; comparator tolerance must not conceal
changes in payload values, timing fields, or canonical identity bytes.

## W01 temporary record/accounting oracle addition

W01 adds `docs/rust-rewrite/w01/oracle/export.py` against accepted main
`cfd7a0af61a22cfd040a7fcd41ad950f82005959` after F05/F07 acceptance. Its
Python3.14.7 source-bound capture uses synthetic research records, injected clock
and ID factories, and pure coverage/accounting functions. It makes no provider
or store requests. W01 owns these migration observations; G08 retires executable
Python before final backend removal. Native mandatory gates read captured JSON
and never execute the exporter. This is not a client-only exception.

The reviewed source manifest grows from 365 to 366 Python paths and from 242 to
243 backend-coupled tooling/test paths. The README capture command is classified
separately under developer oracle invocations; existing mandatory runtime/CI
commands and historical baseline reports are unchanged. The invocation summary
now reports the actual three existing F04/F05/W01 developer rows. F01 fixture
refresh is limited to the new oracle and these three edited inventory documents;
all prior runtime Python and historical baseline hashes remain unchanged.

The first W01 frozen F03 run correctly failed on the newly staged, unclassified
exporter. Its failed source-bound evidence is retained; this explicit inventory
addition closes that failure rather than weakening exact-set validation. W01's
native hardening differences are recorded separately in `../w01/parity.json`.
## F08 transport differences proposed for review

F08 starts at accepted F05 `7cc420f7ffb61127750e77bf6130e225925f9186`.
The [transport parity ledger](../f08/parity.md) and [ADR 006](../../adr/006-adapter-http-bounds.md)
record finite request/body/redirect bounds, unsupported encoding errors, stricter
redirect credential stripping and the global native idle-pool strategy. Native
source-bound socket tests qualify those cases; full adapter/provider and
platform/portal integration parity remains with the named downstream owners.
No Python source or production browser behavior changes in this slice.

## F11 temporary pure-algorithm oracle addition

F11 adds `docs/rust-rewrite/f11/oracle/export.py` against accepted F05 revision
`7cc420f7ffb61127750e77bf6130e225925f9186`. It captures only deterministic
ranking, bounded scholarly grouping and structural URL classification from
synthetic/public inputs. Native F11 CI replays committed JSON without Python,
DNS, providers or shared stores. The exporter is backend-coupled developer
tooling, owned by F11 for evidence and G08 for executable retirement; it is not
a client exception or mandatory native runtime/gate dependency.

The reviewed manifest grows from 365 to 366 Python paths and from 242 to 243
backend-coupled tooling/test paths. Historical F01 baseline identities and
validation reports remain unchanged. F01 source fixture updates cover this
new temporary file and these reviewed inventory documents only; existing
Python runtime source pins are preserved.


## Integrated W01, T01 and F10 source evidence

The W01 branch retains its research exporter and identity capture, accepted T01
retains the MCP exporter, and accepted F10 adds its quota/health exporter. These
are temporary developer-only source oracles. Native acceptance gates consume
captured JSON and do not execute the exporters or call providers; G08 owns their
retirement before backend removal. The exact inventory union contains 370 Python
source paths: 122 backend/runtime, 247 backend-coupled tooling/test and one
client-only path. Its developer-oracle invocation register contains seven rows.

F01 records the final merged inventory-document identities after the narrow
reviewed refresh and adds the accepted F10 exporter blob pin. Existing W01/T01/F10
exporter bytes, Python runtime pins and historical F01 baseline evidence remain
unchanged.

## T09 temporary browser identity oracle and native authority

T09 adds `docs/rust-rewrite/t09/capture.py` against accepted F06/F07 main
`1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`. It records 61 offline actual-source
observations with 11 exact source hashes; native gates replay the JSON and never
execute Python. Independent pinned-Python recapture matched every byte. T09 owns
the evidence and G08 executable retirement. The current union grows from 370 to
371 Python source paths and from 247 to 248 backend-coupled tooling/test paths;
the developer-oracle invocation register grows from seven to eight rows.
Existing runtime Python and prior oracle bytes/pins are unchanged.

T09 uses maintained AWS-LC signature/HMAC/random APIs from the already locked
TLS backend, server-authoritative Valkey time, exact-record session CAS and
explicit typed input bounds. Named source coercion/config/redirect corrections
and browser namespace drain/relogin consequences are recorded in
[the T09 parity ledger](../t09/PARITY.md). No G04/G07 cutover is authorized or
claimed by this module; legacy audit retention and durable workflow idempotency
remain required handoff obligations.
## F09 temporary source oracle

F09 adds one developer-only exporter for synthetic scope, policy, filter, date
and named-topic ordering observations. This is backend-coupled migration tooling,
not a client exception. Native F09 consumes pinned JSON and never executes Python.
G08 retires the exporter before backend removal. The exact inventory becomes
371 Python paths (122 runtime, 248 tooling/test, one client) and eight developer
oracle invocation rows. Existing runtime and baseline hashes are unchanged.


## T04 isolated OAuth reference

T04 adds `docs/rust-rewrite/t04/capture.py` as a temporary, offline, exact-source
migration oracle (T04/G08), not a runtime dependency or gate subprocess. It
captures pinned SDK registration normalization and source provider state locally;
no HTTP endpoint or provider is called. G08 removes it before backend retirement.
The exact Python source union has 371 paths: 122 backend/runtime, 248
backend-coupled tooling/test and one client-only. Existing runtime/oracle bytes
and all historical baseline identities remain unchanged.

## T04 and T09 accepted-main integration

The union retains both independent temporary oracles exactly once: 372 Python
source paths (122 backend/runtime, 249 backend-coupled tooling/test, one client-only)
and nine developer-oracle invocations. T09 now has 76 source observations: the
original 61 plus fifteen integer drain-setting cases. Historical section counts
above describe their original isolated additions; existing runtime/source and
prior oracle bytes remain unchanged. T04 is accepted at `403dfd3720e969fa829f426136bf0f05b0360c56`;
T09 acceptance and combined-source qualification remain separate.
## F09 and T04 integration

Both temporary oracles remain inventoried exactly once. The combined inventory
contains 372 Python sources: 122 backend/runtime, 249 backend-coupled tooling/test
and one client-only source. Historical isolated-addition counts above are retained.

## Accepted F09, T04 and T09 integration

The exact combined inventory has 373 Python source paths: 122 backend/runtime,
250 backend-coupled tooling/test and one client-only, with ten developer-oracle
invocations. F09, T04 and T09 temporary exporters remain inventoried once each;
G08 retains executable retirement ownership. Historical isolated/earlier union
counts above remain historical. All original runtime and source oracle bytes
are unchanged. F09 is accepted at `6ae5e34fed19e8e9da7c19b95eeea894512f0e6d`;
T09 remains in review and requires qualification against the combined source.


## F13 temporary source-capture inventory

F13 adds one optional backend-coupled exporter,
`docs/rust-rewrite/f13/capture.py`, with two documented invocations (forward
source capture and reverse-read of actual native-created snapshot bytes). Native
F13 gates invoke no Python. G08 retires this executable after the rollback window;
immutable source observations remain. All original disposition/invocation rows
are unchanged. This branch now inventories 374 Python sources: 122 runtime,
251 backend-coupled tooling/test and one client-only source, plus 12 temporary
developer-oracle invocation rows. The later normal accepted-F12 integration must
retain its independent exporter and invocation additions as an exact union.
