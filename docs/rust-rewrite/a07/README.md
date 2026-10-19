# A07 native biomedical adapters

The historical implementation baseline is
`1e9dd82d62456fe6f511cc5ade88bae619fac027`, freshly fetched and verified before
work. A01's prerequisite is merged PR #969 (`025ab2f2d50febcb573ba62579eecf7e2d07ef3a`)
with its accepted handoff and ancestry. The A07 checkpoint is now normally merged
with `1bad48bbe4e25e2f60d5b86668e92bbce922fdf0`, retaining F15, W11 and W02.
Historical source pins remain historical; the gate verifies original native/CI
Git blobs separately from live original provider/config/test bytes.

ClinicalTrials, Europe PMC, PubMed, PubChem and UniProt are actual native
implementations with registered factories. They consume shared F08 transport,
SearchAdmission, F06 raw ConfigScalar settings and the accepted SearchContext.
The source-compatible settings bridge preserves absent/blank endpoint defaults
and nonblank override bytes. Actual registry tests verify all five factories,
source categories and specialist cards, science selection, denied admission and
Europe PMC's disabled-by-default policy. Factory availability does not grant
selection. Registration adds no engine-specific orchestrator dispatch; historical catalog
implementation flags remain unchanged. Shared caller-deadline propagation is
described below.

The immutable evidence contains 172 distinct actual source controls. These
preserve the original 91, 25 nonempty-scalar, six numeric and five digit controls,
plus 45 reviewed shape/date controls. Native replay uses actual parsers and
compares ordered methods, URLs, parameters, request counts, adapter-owned headers,
status and complete response/result fields, including actual media. Only measured
latency is excluded as nondeterminism. The 31-case diagnostic ledger individually
records reviewed differences: native/library exception wording and UniProt's
pre-request TypeError supersession under the adapters-never-raise invariant.
Source-authored messages and upstream 429 versus local-denial diagnostics remain
separate. Python-library User-Agent identity and transport-generated headers are
explicit request-comparison exclusions, not claims of byte-identical HTTPX output.

Source iterable behavior, raw scalar configuration, PubMed missing-summary
positions/all authors, and stable UniProt promotion/species ties are covered.
Europe PMC remains opt-in and lite-only, retains provider URLs, attribution and
preprint notices, and does not claim date/paging enforcement or fetch abstracts,
full text or cursors. Actual adapter-to-rank-to-compact tests preserve attribution,
PPR notices and the 300-character snippet. Their PubMed-first URLs differ; no
same-URL attribution replacement is claimed.

PubMed's aggregate deadline covers both requests, decoding and normalization.
Other direct adapters retain per-request network timers and cooperative bounded
normalization without an invented aggregate timer. Caller context bounds all
five through admission, requests and normalization. One 32 MiB invocation budget
charges first-phase scratch, payload projection and ProducedBatch ownership.
Quota failure is terminal Error with no partial successful output. Shared Python
repr/digit/casefold behavior is reused without a Unicode/JSON library fork.

The quota-success fixture uses a 60-second test timeout, giving PubMed its unchanged
90% (54-second) aggregate budget. It verifies quota and transfer behavior; it is not
default-budget throughput or performance evidence. Production defaults, the aggregate deadline
calculation and deadline controls are unchanged.
Provider-sized CID, digit-scan and stable-rank phases yield cooperatively; actual
pending-phase cancellation/deadline tests and removed-checkpoint mutations prove
cleanup. See BOUNDED_RUNTIME.md for allocation measurements and exclusions.

Parent review accepted the shared payload/batch, repr/scalar/casefold,
factory/gate/metadata, runtime and main-preservation components. The integrated
F03 workspace gate passed on `bff188e33b43c2d4e82bc61fa3dfea48c1d7b56c`: source and
inventory checks, formatting, strict Clippy, workspace tests/build and native
readiness. The integrated A07 suite includes 71 infrastructure units, 20 biomedical
tests, seven context tests, seven factory tests, two compact consumers and four
gate rejection tests. Exact frozen reports govern their source/log identities.
Final scoped documentation-head reports and exact-head PR/CI are required delivery evidence.

CI runs the real A07 gate and retains its source-bound report. The normal main
union preserves 399 tracked Python source entries and 40 historical oracle
invocations. Twelve A07 producers are explicitly inventoried for G08 retirement;
the native A07 runtime/gate neither invokes nor requires them. Captures and their
source provenance remain immutable. G02 global/query-input/DOM/returned-response
accounting and T03/G01/T06 HTML/full-MCP integration remain genuine downstream
boundaries; see PORTAL_IMPACT.md and REMAINING_A07.md. No live provider calls,
production rollout or complete-milestone acceptance is claimed.
