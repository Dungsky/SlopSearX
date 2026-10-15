# ADR 005: Native protocol, TLS and platform contract

- **Status:** Proposed for F02 maintainer acceptance
- **Scope:** Rust Rewrite HTTP/MCP surfaces, gateway/client binaries and release targets
- **Decision authority:** Maintainer direction in the Rust rewrite case study and issue #881; acceptance is the merged F02 PR
- **Date:** 2026-10-15
- **Depends on:** ADR 003 and accepted F01 surface inventory

## Context

F01 records four search routes (`GET`/form `POST` at `/` and `/search`), HTML/JSON/YAML/CSV/RSS, readiness/config/metrics, workflow/OIDC routes, and the full MCP registry. The runtime supports stdio, streamable HTTP and legacy HTTP+SSE. The official Rust MCP SDK `rmcp` 3.5.1 documents stdio and streamable HTTP transports, but no built-in SSE pair. The current FastMCP 4.0.11/MCP 2.3.0 SSE route has GET/HEAD `/sse`, a POST message endpoint under `/messages`, session IDs and specific acceptance/error behavior. Its CLI branch is currently unauthenticated. The gateway is a local stdio MCP server proxying the remote streamable HTTP catalog; prior fixes preserve exact input schemas and the upstream session lifetime. Availability of a Rust crate alone does not prove those contracts.

## Decision: preserve all advertised transports

Use `rmcp` for common MCP messages, stdio and streamable HTTP, with a dedicated Axum/Tower legacy SSE transport shim that maps session-bound POST messages and the event stream to the same MCP handler registry. Do not describe SSE as implemented by `rmcp`. The shim must preserve path customization, GET/HEAD semantics, `text/event-stream`, `/messages` advertised prefix and accepted suffix behavior, session creation/expiry, JSON-RPC errors and status behavior from F01; T01 owns real-client qualification against the pinned Python baseline before T06 considers it implemented. Keep the current SSE exposure policy as a recorded parity property; adding authentication or retiring SSE requires a separate accepted intentional-difference decision and a migration path. Static bearer, OAuth server and unauthenticated HTTP modes remain distinct; the native server cannot silently apply MCP OAuth client code to authorization-server endpoints. MCP OAuth protected-resource/authorization/registration/token/revoke routes and their persistent state belong to T04/T05. Browser OIDC is separate and remains owned by T09.

Tool definitions and JSON Schemas are explicit checked-in contract descriptors generated or validated from native handlers, not incidental Rust signature output. Preserve the ordered 38 names, required/default/optional fields, descriptions, error precedence, structured content and null/empty distinctions, four resources and five prompts. Gateway discovery paginates every tool/resource/prompt catalog, republishes each remote input schema verbatim, forwards resource MIME/text/blob metadata and prompt messages, and holds the remote session for the full local stdio lifetime. Unknown remote errors remain errors. The shared application policy is invoked by each dispatch-capable tool path, including resume/revalidation; read-only tools and public HTTP retain their accepted surface-specific policy inputs. Any tightening of existing HTTP authority must be recorded as an intentional difference, not smuggled in as a Rust type check.

HTTP continues to use one application service and wire DTOs independent of domain structs. Search format negotiation, GET/form POST equivalence, partial results, `health` versus `healthz`, OpenMetrics text, SearXNG JSON and portal security headers remain testable at real native routes. The protected workflow console is rendered server-side and uses its own browser session, tenant, CSRF/origin and revision/idempotency checks. Public search does not require or infer identity from that cookie.

## TLS, trust roots and host authority

Two deployment boundaries are intentionally different. Ordinary public search may remain behind an operator TLS proxy under ADR 001. That proxy cannot become a trusted identity path for the protected console. **Protected workflow OIDC requires direct HTTPS at the application** under ADR 002: use rustls/Tokio TLS at the native listener, a configured HTTPS external origin, Host allowlist and exact origin/callback checks. Ignore forwarding headers and reject proxy-trust configuration for protected routes until a superseding identity ADR accepts a narrowly trusted proxy mode. Loopback HTTP development may be explicitly configured without enabling production OIDC; no off-loopback OAuth/browser secret flow over cleartext. MCP remote OAuth issuer and gateway callback require browser-reachable HTTPS outside loopback. Static bearer and OAuth credentials are never logged or sent to a different origin through redirects.

Use reqwest 0.13.5 with its documented default rustls/platform-verifier path for outbound provider, IdP and gateway HTTPS. Accept an operator-supplied PEM CA bundle as additive trust: parse every certificate, pass the collection to [`ClientBuilder::tls_certs_merge()`](https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html#method.tls_certs_merge), and fail startup for the affected protected integration if the bundle is missing/malformed, the selected verifier cannot merge roots, or client construction fails. The older `add_root_certificate()` method is deprecated in this pin. Cargo features are additive across dependencies, so F03/T01 must inspect the locked feature graph and verify the selected TLS backend and platform roots rather than infer them from one manifest entry. Do not disable hostname/certificate verification. Root loading, system-proxy behavior, private CA, mTLS if required by a provider, and redirect/auth-header stripping are platform tests, not assumptions. Server TLS uses rustls 0.23.46 with tokio-rustls 0.26.6; certificate/key reload and TLS termination operations are O02/T09 work, and a static pinned candidate is not proof of a working listener.

## Supported targets and pins

The existing **published** container platform set is Linux `amd64` and `arm64`; F02 preserves those as mandatory server, worker and MCP server image targets. Native `ssx` and gateway client binaries target Linux x86_64/aarch64, macOS x86_64/aarch64 and Windows x86_64 as a deliberate new distribution goal. macOS/Windows are not current backend guarantees. A client target becomes supported only after O04 clean-install, TLS/CA, invocation and artifact-signature tests on that target; unavailable targets stay marked pending rather than advertised. Container images are non-root, carry required CA/TLS assets, have health/signal behavior and immutable digest promotion. Linux `musl`, 32-bit targets and native macOS/Windows **servers** are outside the initial release matrix unless separately accepted with equivalent gates.

The candidate toolchain is stable Rust **1.99.0**, confirmed by the [Rust release announcement](https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/). F03 commits `rust-toolchain.toml`, exact Cargo manifest requirements and `Cargo.lock`; native builds use `--locked`. F03 checks the foundation on its available Linux host. O04 later builds and qualifies every shipping target after the server, worker, gateway and client crates exist. These are **candidate exact direct-dependency pins**, verified against current primary crate documentation on 2026-10-15, not a claim that the combined graph already compiles:

| Purpose | Candidate exact version | Primary source |
| --- | --- | --- |
| Tokio runtime | `tokio =1.53.3` | [Tokio crate](https://docs.rs/tokio/1.53.3/tokio/) |
| HTTP router | `axum =0.8.9` | [Axum crate](https://docs.rs/axum/0.8.9/axum/) |
| Middleware | `tower =0.5.3`, `tower-http =0.7.1` | [Tower](https://docs.rs/tower/0.5.3/tower/), [tower-http](https://docs.rs/tower-http/0.7.1/tower_http/) |
| Outbound HTTP/TLS | `reqwest =0.13.5`, `rustls =0.23.46`, `tokio-rustls =0.26.6` | [reqwest](https://docs.rs/reqwest/0.13.5/reqwest/), [rustls](https://docs.rs/rustls/0.23.46/rustls/), [tokio-rustls](https://docs.rs/tokio-rustls/0.26.6/tokio_rustls/) |
| MCP protocol | `rmcp =3.5.1` | [Official MCP Rust SDK crate](https://docs.rs/rmcp/3.5.1/rmcp/) |
| Valkey RESP client | `redis =1.7.1` | [redis crate](https://docs.rs/redis/1.7.1/redis/) |
| Serde/schema | `serde =1.0.229`, `serde_json =1.0.154`, `schemars =1.2.3` | [serde](https://docs.rs/serde/1.0.229/serde/), [serde_json](https://docs.rs/serde_json/1.0.154/serde_json/), [schemars](https://docs.rs/schemars/1.2.3/schemars/) |
| Provider parsing | `scraper =0.27.0`, `quick-xml =0.42.0` | [scraper](https://docs.rs/scraper/0.27.0/scraper/), [quick-xml](https://docs.rs/quick-xml/0.42.0/quick_xml/) |
| Native CLI | `clap =4.6.7` | [clap](https://docs.rs/clap/4.6.7/clap/) |

`=version` means an exact Cargo direct dependency requirement, not caret compatibility. F03/T01 must resolve the graph, inspect MSRV/license/advisories and compile the foundation/protocol on their available target before treating those pins as frozen. O04 compiles and tests every mandatory release target before publication. If incompatibility requires a change, amend this ADR/pin table and record the reason before dependent implementation. Adapter-specific parser and crypto/OIDC crates are selected with their owning issues after source-specific fixtures and security review; their absence here is not permission to bypass those contracts. No Python interpreter, embedded CPython, PyO3 backend, sidecar or remote Python fallback is part of a released target.

## Qualification and unresolved risks

| Risk | Accountable role | Required gate |
| --- | --- | --- |
| rmcp transport/schema behavior differs from FastMCP, especially SSE | Protocol owner | T01 pinned-client descriptor and stdio/HTTP/SSE exchange matrix, including invalid sessions/errors, before T05/T06 |
| SSE baseline has no dedicated test and is unauthenticated | Protocol/security owners | T01 explicit parity fixture, G02 exposure review; new auth requires an accepted difference |
| Rust TLS roots or proxy behavior differs by target | Platform/security owners | O02/O04 private/public CA and redirect tests; T09 direct-TLS/Host/origin tests |
| Client binaries lack current baseline support on macOS/Windows | Release owner | O04 target-specific install/smoke/signature gate before publication |
| OAuth authorization server and relying party contracts diverge | Identity owner | T04/T05/T09 multi-process fault/security matrix, G02 |
| Exact crate graph or target build fails | Foundation owner | F03 `cargo build --locked`/Clippy and T01 protocol qualification; ADR amendment if pins change |
