# A07 biomedical semantic/transport candidate

Baseline `1e9dd82d62456fe6f511cc5ade88bae619fac027` was freshly fetched and
verified equal to main before work. A01 prerequisite is merged PR #969
(`025ab2f2d50febcb573ba62579eecf7e2d07ef3a`) with accepted handoff and ancestry.

The five native modules consume accepted F08 sessions, bounded source JSON,
F06 raw ConfigScalar values and SearchEngine/SearchAdmission. ClinicalTrials
owns batch-private mechanics; shared transport/config/Unicode/JSON remain
separately owned. Factory activation remains pending review.

The first 91 controls, 25 nonempty raw-scalar controls, 6 PubMed numeric controls
and 5 Unicode digit controls remain immutable. The complete capture preserves
all 127 original inputs, ordered requests and prior outputs while adding engine
set/tier/work_group and response answers/corrections/infoboxes/synthetic. Further
45 reviewed shape controls cover sequence/empty-iterable behavior and ISO dates.
All 172 distinct controls run through actual native parsers. Only measured
latency is omitted from equality as nondeterminism.

Captures carry pinned source SHA256/Git blob identities and actual requests and
outputs. Exporters used only HTTPX MockTransport. Original inline mocks are
labeled synthetic; preexisting Europe PMC recordings retain exact bytes.
Temporary Python exporters are historical evidence, never native/runtime gate
dependencies; their inventory/retirement changes are proposed separately. Scoped
Ruff passes. Formatting/import edits preserve exporter AST behavior; removing
dead empty-iterable setup reproduces byte-identical output under Python -O.

Native replay compares ordered methods/URLs/counts, adapter-owned headers,
status and complete result/response fields. Every diagnostic difference is
individually fixed in proposed-differences.json: accepted Python-runtime/HTTPX
wording and the source UniProt pre-try TypeError → classified Error correction.
Source-authored PubMed/Europe PMC messages remain exact. Upstream 429/timeout
absence remains distinct from local denial Some("rate limited"). Accept and
explicit PubMed/UniProt User-Agent headers are exact; only Python-library
User-Agent identity is excluded. Host/Connection/Accept-Encoding remain F08
transport concerns rather than adapter-injected fields.

Source iterable semantics are preserved without permissive array coercion:
empty sliced strings contain no malformed rows; maps still reject slicing;
PubChem empty props strings/maps yield fallback compounds; explicitly validated
PubMed/Europe PMC list fields still reject strings/maps. Native tests preserve
all related UniProt records/species ties, bounded exact-match promotion,
PubMed missing-summary positions and all six authors versus three in display.

Europe PMC remains opt-in/lite-only, retains provider URLs/attribution/PPR notice
and does not follow cursors, fetch abstracts/fulltext or claim date/paging
filters. Actual adapter-to-rank/compact tests now qualify attribution/PPR and the
300-Unicode-character snippet; the PubMed-first fixture has distinct URLs and
does not prove same-URL attribution merging. HTML integration remains T03/G01.

PubMed alone uses an aggregate deadline for ESearch, ESummary, source decoding
and normalization checkpoints. The other four retain per-request network
timeouts and cooperative bounded source decoding, without inventing an
aggregate direct-adapter deadline. The deterministic deadline unit distinguishes
expired network admission, PubMed expired parse budget and source resource
bounds when no aggregate parse budget is supplied. Row-level yields do not
qualify one large inner metadata loop as bounded work.

Raw publication payload projection now uses the accepted charged visitor and
ProducedBatch at runtime, under a shared 32 MiB row-output/scratch root. The new
shared cooperative source_repr visitor and borrowed casefold scalar exposure
remove the provisional infallible repr allocation path. See BOUNDED_RUNTIME.md
for precise ownership, parser/input exclusions, quota hardening, phase evidence,
measurements and remaining parent integration; this remains a candidate, not
factory/global-memory/portal readiness.

Validation includes 20 native tests / 172 original controls, seven context tests,
two actual adapter→rank→compact tests, 14 real resource tests over eight prepared
bodies, 12 accepted payload/batch tests, seven shared repr tests with 35 actual
Python controls, shared domain/JSON tests, strict Clippy and formatting. The
freeze receipt is the authority for terminal commands/source/log identities.
Original captures and the 31-case diagnostic ledger remain unchanged. Temporary
Python evidence is proposed for explicit inventory registration/G08 retirement;
no runtime or required native gate invokes it.
