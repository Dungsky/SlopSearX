# F04 replay corpus

`crates/xtask/fixtures/replay/corpus.json` contains five portable observations: npm, crates.io and OpenAlex adapter exchanges; a SearXNG JSON search response; and an MCP search response with structured filter enforcement. The three adapter cases replay exact request order and sanitized HTTP responses through a loopback-only server. The two surface cases execute the pinned application and MCP harness with deterministic fake engines and an in-memory store. All five expected/candidate envelopes were captured from two independent Python source executions. They are source observations, not Rust parity results.

The surface case IDs are `search-http-json-contract` and `mcp-search-filter-enforcement`. The MCP case requests `language=de`, `time_range=week`, and `safesearch=moderate`; its complete enforcement report is retained and currently reports `unsupported` for each filter because the fake engine enforces none. The search HTTP case retains the complete SearXNG JSON result and metadata schema. The adapter results use the complete `SearchResult` dataclass shape and sorted engine provenance.

The corpus records baseline Git blob IDs for all Python adapters, fixtures, application surfaces and source tests. `provenance.oracle` records the developer exporter path and SHA-256. The only normalized values are measured latency and response time, replaced with numeric zero during export to make the corpus byte-stable. The Rust comparator still validates their numeric types and non-negative values. Status, schema, enforcement, provenance, counts, result order, null/omission, request details and all other values remain exact.

The response bodies use the existing reviewed upstream fixtures at `tests/fixtures/upstream_contracts/{npm,crates,openalex}.json`. Only their allowlisted public parser fields are retained; no identity headers, credentials, author/publisher identities, raw errors or unfiltered payloads are included. Corpus structure and generated observations are MIT-licensed with this repository. The embedded sanitized upstream API facts retain their source terms; this corpus does not claim an additional license grant.

## Export and replay

The exporter runs pinned Python source from a clean detached checkout and writes output into the current checkout. From the repository root:

```sh
git worktree add --detach /private/tmp/slopsearx-f04-oracle-src 7b2d876307189d7a62000d1f289b3ef4b65eaff6
.venv/bin/python docs/rust-rewrite/replay/oracle/export.py --source-root /private/tmp/slopsearx-f04-oracle-src --baseline 7b2d876307189d7a62000d1f289b3ef4b65eaff6
cargo xtask check F04
```

The exporter rejects a dirty or wrong-revision source checkout, including untracked files, strips provider configuration from its environment, blocks DNS and socket connections to non-loopback hosts, and uses only sanitized recorded responses, an in-process ASGI test client, and the local MCP fixture server. It does not call live or paid providers. It verifies each fixture source blob against the recorded baseline before writing the corpus.

`cargo xtask check F04` verifies source blob provenance, the deterministic input/clock/ID fields, the exact transcript and request count/order, envelope schemas and the full expected/candidate comparison. The MCP check requires the `enforcement` object and its field-level statuses. The check must fail for changed status, schema, provenance, request count/order, filter enforcement, result order, omitted/null changes, or non-volatile values.

When a native candidate exists, run it against one case with:

```sh
cargo xtask replay-run crates/xtask/fixtures/replay/corpus.json mcp-search-filter-enforcement -- ./path/to/native-candidate
```

`replay-run` supplies `SSX_REPLAY_URL`, `SSX_REPLAY_ORIGIN`, `SSX_FIXED_UNIX_SECONDS`, `SSX_FIXED_IDS`, and `SSX_REPLAY_INPUT`. The candidate must print one `{"status": ..., "body": ...}` observation to stdout; stdout is parsed and compared to the pinned Python `expected` envelope. Its HTTP requests are served by the local transcript and checked for origin, method, target, headers, body, count and order. The current workspace has no completed native search or MCP candidate, so this command is an interface for later ports and currently demonstrates no Rust parity.

The exporter command is optional developer tooling. Python is never invoked by Rust runtime, native replay, mandatory native CI/release gates, deployment, scheduled jobs or active backend operation. The corpus provides source observations to compare against native implementations; it does not certify that any Rust adapter or surface is at parity until native candidate outputs are supplied and the differential gate passes.

## Refresh procedure

1. Start with a clean detached worktree at the recorded Python baseline. The exporter requires the source commit and rejects tracked modifications.
2. Do not update observations in response to a failure. First determine whether the source, fixture, or export path changed. Any accepted behavior change belongs in the Rust rewrite parity ledger.
3. For upstream cases, use only the already-reviewed sanitized fixtures unless a separately reviewed bounded capture is authorized. Never include auth, cookies, personal fields, proxy headers or raw provider exceptions.
4. Preserve exact methods, relative targets and query strings, response status/headers/body, full result order, complete output fields and provenance. Add only a specifically justified numeric volatility pointer from the comparator allowlist.
5. Inspect the generated corpus diff and source blob list before merging. The fixed exporter command must reproduce the corpus byte-for-byte at the same source and exporter revisions.

## Temporary Python oracle boundary

The exporter imports the historical backend only in a developer process. It consumes fixture outputs and writes JSON; it does not bridge Python into Rust or provide a fallback runtime. Rust adapter and surface work must retain native tests and compare their actual outputs against this corpus. Once all consumers use accepted native evidence, Python oracle invocation can be retired while keeping the historical fixture bytes and their provenance. This path adds one developer-only Python file, `docs/rust-rewrite/replay/oracle/export.py`, for inclusion in the rewrite's Python invocation/source inventory.
