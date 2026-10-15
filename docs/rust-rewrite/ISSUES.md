# Rust rewrite issue index

Publication IDs are populated after the committed case study is available. The [plan](plan.json) defines hard finish-to-start dependencies; the [case study](CASE_STUDY.md) defines scope and completion.

| Plan ID | Work package | Lane | Must finish first |
| --- | --- | --- | --- |
| F01 | Freeze parity inventory and the Python removal boundary | contracts | None |
| F02 | Decide Rust crate boundaries, storage compatibility and transport policy | contracts | F01 |
| F03 | Add Cargo workspace and minimal native CI smoke | foundation | F02 |
| F04 | Build portable fixture and differential replay harness | contracts | F03 |
| F05 | Implement typed domain records and stable serialization | foundation | F04 |
| F06 | Port configuration and generate the adapter capability registry | foundation | F05 |
| F07 | Implement Valkey access, atomic ownership and failure types | state | F05 |
| F08 | Implement pooled HTTP transport with isolated search sessions | adapters | F05 |
| F09 | Port fail-closed policy, filter reports and scope resolution | search | F06 |
| F10 | Port distributed rate limits, cooldowns and circuit health | search | F07, F08 |
| F11 | Port deterministic ranking and scholarly grouping | search | F05 |
| F12 | Port optional Jev routing and deterministic fallback | search | F08, F09, F07 |
| F13 | Port canonical cache and tenant-bound snapshots | state | F07, F09 |
| F14 | Deliver integrated search execution with native fake engines | search | F10, F11, F12, F13 |
| F15 | Port bounded semantic reranking and result advisories | search | F14 |
| T01 | Qualify Rust MCP SDK schemas, transports and lifecycle | protocol | F04 |
| O01 | Port metrics, audit, tracing and graceful service supervision | operations | F14, F06 |
| T02 | Serve compatible HTTP routes and machine formats in Rust | http | F14, F15, O01 |
| T03 | Port public search portal and browser acceptance journeys | portal | T02 |
| T04 | Implement authoritative OAuth state and atomic token transitions | identity | F07, F06 |
| T05 | Implement OAuth authorization endpoints and bearer enforcement | identity | T01, T04 |
| T06 | Expose search, discovery and snapshot MCP contracts | protocol | T05, F15 |
| T07 | Ship native MCP gateway and OAuth client | clients | T06 |
| T08 | Replace ssx with a native Rust CLI | clients | T02 |
| T09 | Port browser OIDC, sessions and tenant authorization | identity | F07, F06 |
| W01 | Port research records and conserved budget accounting | workflows | F07, F05 |
| W02 | Run durable research jobs and recovery in Rust | workflows | W01, F15 |
| W03 | Port caller-directed adaptive research planning | workflows | W02, T06 |
| W00 | Port saved definitions, revisions and schedule claims | workflows | F07, F09 |
| W04 | Port saved execution, observations and atomic reports | workflows | W00, F15 |
| W05 | Port saved event outbox, acknowledgements and retention | workflows | W04 |
| W11 | Port staged admission, records and retry transitions | workflows | F07, F09 |
| W06 | Run staged searches with bounded fallback and recovery | workflows | W11, W02, W04, F15 |
| W07 | Port retrieval receipts, manifests and artifact lineage | evidence | F13, F07 |
| W08 | Port dependency dossiers and workflow composition | evidence | W07, F15, W02, W04, W06 |
| W09 | Complete workflow MCP tools, prompts and registry parity | protocol | W03, W05, W06, W08, T06, T07 |
| W10 | Port authenticated workflow console and browser mutations | portal | T09, T03, W09 |
| A01 | Port registry-search pilot adapters | adapters | F06, F08, F04 |
| A06 | Port scholarly publication sources adapters | adapters | A01 |
| O02 | Replace Python health, deployment and upstream-contract tools | operations | T02, F08, A01, A06 |
| O03 | Classify active experiment tooling and define native evidence contracts | tooling | F04, F14 |
| O04 | Build native release artifacts, containers and install paths | operations | T07, T08, O01, O02, W09 |
| A02 | Port package registries adapters | adapters | A01 |
| A03 | Port developer and community sources adapters | adapters | A01 |
| A04 | Port broad api search adapters | adapters | A01 |
| A05 | Port html general search adapters | adapters | A01 |
| A07 | Port biomedical metadata sources adapters | adapters | A01 |
| A08 | Port regulatory financial and legal sources adapters | adapters | A01 |
| A09 | Port catalog geocoding and archive sources adapters | adapters | A01 |
| A10 | Port vulnerability and exploit knowledge adapters | adapters | A01 |
| A11 | Port ip and url intelligence adapters | adapters | A01 |
| A12 | Port infrastructure security enrichment adapters | adapters | A01 |
| A13 | Port sensitive and asynchronous intelligence adapters | adapters | A01, F09 |
| A14 | Port ats jobs and company targeting adapters | adapters | A01 |
| O05 | Replace backend-importing tests and isolate optional Python clients | quality | W09, T03, T07, T08, W10, A01, A02, A03, A04, A05, A06, A07, A08, A09, A10, A11, A12, A13, A14 |
| O06 | Prepare native CI and protected review successors | tooling | F03, F01 |
| G01 | Prove integrated full-feature and all-adapter parity | qualification | A01, A02, A03, A04, A05, A06, A07, A08, A09, A10, A11, A12, A13, A14, W10, F15, T07, T08, O01, O04, O05 |
| G02 | Qualify security, multi-process recovery and overload behavior | qualification | W10, O01, T05 |
| G03 | Measure native resource and performance outcomes | qualification | G01, G02 |
| G04 | Implement and rehearse durable-state migration and rollback | state | W09, T05, T09 |
| O07 | Port backend-coupled acquisition and replay drivers | tooling | O03, W09 |
| O08 | Port evidence receipts, qualification and verification tools | tooling | O03, O07 |
| O09 | Switch required CI and release gates to native implementations | tooling | O05, O02, O08, O06 |
| G05 | Publish native operator and contributor migration runbooks | operations | O04, O06, G04, O08, O09 |
| G06 | Rehearse the exact release in isolated staging | release | G01, G02, G03, G05 |
| G07 | Canary and cut over production to native services | release | G06 |
| G08 | Remove all backend Python source and runtime dependencies | retirement | G07, O05, O06, O08, O09 |
| G09 | Verify final Rust-only release and close rewrite acceptance | release | G08 |
