# T09 browser identity, sessions and tenant boundary

T09 implements dedicated browser OIDC and opaque server sessions, independently
of machine OAuth. Baseline is accepted F06/F07 main
`1f3f5bc85083f4fd24bd9a2b07f161626e6bfef5`. F07 supplies bounded native Valkey
connections and explicit unknown outcomes; F06 supplies redacted operator
secrets. No unaccepted T04/F09 interface is imported.

## Ownership and integration

- `ssx-config::browser_identity::BrowserConfig` parses explicit operator inputs
  and bounded membership JSON. Its secrets use the accepted F06 `Secret`.
  Membership is process configuration, not another Valkey directory.
- `ssx-domain::browser_identity` owns principal/membership/session/context values,
  opaque nonserializable `BrowserSecret`, closed actions/reasons and errors.
- `ssx-application::browser_identity` owns directory, identity and workflow ports.
  Every principal lookup and verified external tuple is checked against the
  directory response. Effective grants intersect live membership/operator grants.
- `ssx-infra::browser_identity::BrowserAuthority` performs real HTTPS OIDC
  discovery/code exchange and maintained signature verification. Browser-specific
  Valkey scripts own one-time transactions, exact-record session CAS, admission,
  quotas and bounded pseudonymous audit streams.
- `ssx-http::browser_identity` supplies actual Axum auth/workflow boundary routes,
  cookie/Origin/CSRF/form checks and a direct TLS listener. At most 16 handshakes
  are polled concurrently as owned futures; no detached handshake tasks exist.
  Excess peers are refused; dropping the listener closes pending peers.

The composition root explicitly mounts `browser_router` with injected identity
and workflow ports and serves it through `BrowserTlsListener` with
`into_make_service_with_connect_info::<BrowserTlsPeer>()`. Other listeners cannot
supply that private peer marker; protected routes deny without direct TLS.
Forwarding headers are ignored and trusted-proxy configuration is rejected.
The configured HTTPS Host/origin/callback remain authoritative. Public routes
are mounted outside this boundary and do not resolve sessions.

W10 owns real workflow rendering and durable commands, including object tenant,
current application policy, expected revision and durable operation idempotency.
The T09 browser admission guard wraps that command exactly once: pending/done
records both retain 600 seconds, exact request/claim CAS fences completion, done
replay redirects without invoking the command, and pending/uncertain attempts
cannot be reissued by the browser guard. W10 must not double-claim this browser
admission. Guard expiry does **not** prove a workflow operation did not happen;
W10's durable idempotency/reconciliation remains necessary beyond this horizon.
Only semantic cursor/question/strategy fields cross into W10, with source
strategy default `triangulate`; CSRF/cookies never enter its command or Debug.
The source NUL-joined semantic digest excludes CSRF and idempotency presentation.

No production startup/cutover switch, full W10 workflow UI or T02 public search
implementation is claimed here. Integration tests mount independent public
stand-ins to prove cookie/session middleware does not change public responses.
T09 must be composed with those accepted surfaces later; it does not replace
search or grant provider access.

## Security and lifecycle

Login uses random 256-bit state, nonce, verifier and browser handle. The public
redirect contains only the PKCE challenge, never the verifier. The transaction
binds local return path/state/nonce/verifier, expires after 600 seconds and is
consumed with exact raw CAS before the token request. A lost token response never
retries the code exchange. Failed state/duplicate callback input does not consume
or dispatch; replay after successful consumption cannot dispatch again.

The compact JWT is bounded before decoding:32 KiB total,2 KiB encoded header,
24 KiB encoded claims and bounded RSA components/signature. JSON is at most 64 KiB,
32 levels, 256 object fields and 1024 array members, rejecting decoded duplicate keys
at every depth. Only fixed RS256 is accepted, with a unique matching `kid`, RSA
key type and compatible `use`/`alg`/nonempty verify-only `key_ops`. `crit` and `b64`
extensions are refused. Maintained AWS-LC verifies RSA 2048–8192; no handwritten
RSA, padding, HMAC or random implementation was ported. Issuer, audience, required
multi-audience `azp`, nonce, subject and finite numeric time claims are explicit;
source 30-second skew is retained. Source permits irrelevant single-audience
`azp`, and that rule remains unchanged. Metadata/JWKS cache lasts for the authority
instance, matching source; automatic IdP signing-key cache refresh is not claimed.

Valkey TIME supplies persisted timestamps, expiry and quota slots. Sessions retain
source 30-minute idle/eight-hour absolute defaults (tests shorten within accepted
bounds); activity never extends absolute expiry. Principal/membership revision
mismatch revokes via exact observed-value deletion, so stale refresh cannot
revive or delete a newer record. Tenant switching rotates the random handle and
CSRF secret atomically; stale handles and stale rotation/finish operations deny.
Previous HMAC keys verify only during a configured 60–600 second server-time drain;
new handles issue under the current key. No raw OIDC token is stored in sessions.

The cookie is `__Host-slopsearx_session; Secure; HttpOnly; SameSite=Lax; Path=/`
with no Domain. Login transaction cookie is source-compatible
`__Secure-slopsearx_login` on `/auth/callback`, max-age 600. Logout is POST+CSRF,
invalidates the server session and clears the cookie even without IdP logout.
There is no production cleartext/development-cookie mode in this module.

Mutation validation requires exact same-origin Origin (same-origin Referer only
when Origin is absent), a bounded URL-encoded form and synchronizer token verified
in constant time. ADR002's `X-CSRF-Token` alternative is supported; conflicting or
duplicate credential inputs deny. Tenant choices come only from live directory
membership. Authenticated unavailable objects share the same 404 body/security
headers; HTML authentication redirects 303 while machine requests receive 401 and
`WWW-Authenticate`. Unsupported action/kind is source 403, not an object lookup.

Closed rate buckets preserve source limits: login 10/60 seconds, callback 20/600 seconds,
read 120/60 seconds, mutation/tenant switch/logout 30/60 seconds, with framed pseudonymous identities
and server-authoritative slots. Counters increment+expire atomically. Audit uses
closed action/outcome/reason vocabularies, keyed pseudonyms, exactly 10,000 entries per
day and 90-day retention. Cookies, CSRF, tokens, subject and workflow contents are
excluded. Connected audit-write failure denies successful protected responses;
a completed mutation remains completed and must not be repeated. Login/rotation
withholds an unlogged cookie and revokes the unissued handle best effort.

## Native evidence

`cargo xtask check T09 --output target/rust-evidence/T09.json` binds immutable
reviewed capture/exporter digests, exact accepted Python bytes and exact native
test inventories, failing on missing/ignored/empty tests or forged expectation
metadata. It runs actual TLS loopback fake-IdP/browser clients and official private
Valkey 8.1.10, two independent authorities, forged authentic-claim negatives,
expired/revoked/corrupt records, stale CAS deletion/rotation, admission replay,
CSRF/Host/Origin/forms, stalled TLS/drop cleanup, namespace isolation, audit bounds,
and an actual committed EVAL with lost reply proving `CompletionUnknown`/one dispatch.
Compile-fail evidence prevents generic secret serialization.

Temporary `capture.py` uses pinned Python 3.14.7, asserts 11 source file identities
against the accepted baseline and records 61 offline observations. It is never
invoked by native gates/runtime. Native tests consume every source observation
with exact named intentional-difference branches; G08 retires the exporter.
Independent recapture matched corpus SHA
`d9c4453669499ebe8b6da05d3e93037d3b96dfd7ad591f4470d2642fd933e5b9`;
parent proof is `/private/tmp/t09-parent-recapture-attempt1/parent-verification.json`.
Capture presence or counts alone are not runtime parity evidence.

## Cutover and downstream handoff

New keys are `portal:native:v1:<sha256(issuer,client,callback)>:<kind>:<keyed id>`.
Native readers never silently adopt legacy browser transactions/sessions. This
module is not wired into production by default. G04/G07 must stop legacy admission
and writers, drain/reconcile pending browser/workflow commands, expire legacy
login/session authority and require sign-in again before the exclusive native
writer starts. Existing audit evidence retains its original namespace/retention;
G04 must preserve/export it, not discard it as session state. Rollback likewise
stops native admission, reconciles workflow results and requires fresh legacy
sign-in; native handles are not rollback-compatible cookies. No runtime dual
writer, automatic migration or successful G04/G07 rehearsal is claimed.

G02 owns adversarial release qualification, W10 the real workflow/UI integration,
O01 audit operational integration and G08 temporary Python retirement. See
[PARITY.md](PARITY.md) for exact source behavior and reviewed corrections.
