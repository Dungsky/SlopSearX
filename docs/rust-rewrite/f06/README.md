# F06 implementation evidence

F06 owns the static config/catalog seam; later adapter batches provide engine-local factory registrations and metadata changes. New provider ports must update only their engine fragment and provider crate files. The catalog, runtime lookup, and documentation aggregate are generated or validated centrally.

`cargo xtask check F06` is the focused acceptance gate. It checks pinned Python source blobs, fragment-to-catalog equality, exact per-engine Jev roles and routing-card text from the separately frozen source-derived `routing-expectations.json`, a same-count card-swap negative case, descriptor completeness, the 51-engine set, generated Markdown, source-case coverage, and native config/registry tests. The frozen fixture records the `slopsearx/jev.py` Git blob from `source-pins.json`; aggregate role counts alone are not considered parity evidence. It does not contact providers, use live credentials, or claim that any provider has been ported.

Engine factories are registered against descriptors in the registry crate. The application/engine wiring layer owns an immutable application context and engine-local resolved configuration, which a factory can capture through `Arc`; the registry does not depend on configuration or application crates. Adding a provider therefore remains an engine-local registration and does not require an orchestrator edit.

The source-backed compatibility record and the outstanding approval-blocked credential case are in [PARITY.md](PARITY.md). The current aggregate is [catalog.json](catalog.json), with one JSON descriptor per engine under `fragments/` and [generated catalog documentation](engine-catalog.md).

`crates/xtask/fixtures/f06/scalars.json` contains 21 actual Python 3.14.7
`load_config` observations against the exact pinned config source: raw scalar
kinds/values, numeric grammar, None-default text, YAML-origin coercion, ordered
alias resolution, list stripping and expected errors. The native config tests
replay those inputs; F06 independently pins the exact fixture SHA256 and rejects
changed answers. No Python helper is invoked by the native gate. The capture
uses only noncredential settings and leaves the blocked credential regression
visible. Raw `ConfigScalar` values require `effective_i64`/`effective_f64` plus
consumer-specific bounds before use by later native adapters.
