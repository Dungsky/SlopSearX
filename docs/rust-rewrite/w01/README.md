# W01 research records and conserved accounting

Baseline: `cfd7a0af61a22cfd040a7fcd41ad950f82005959`, with accepted F05/F07.
Issue: [#905](https://github.com/magnus919/SlopSearX/issues/905).

`ssx_domain::research` owns the durable job/query/attempt/coverage schema and pure
budget transitions. Completed attempts contain their own cursor, query identity,
coverage, enforcement, admitted result IDs and newly seen lead IDs. Mutable
execution ownership lives on the job and is supplied independently as a typed
`LeaseToken<JobId>` to every authoritative mutation. Candidate records can be
edited in memory; the store rejects any rewrite or deletion of completed attempt
history or query planning provenance before an atomic write.

`ResearchJob::from_payload` fills source legacy defaults and normalizes empty/null
collections only at declared schema paths. It retains large integers and nulls in
opaque workflow/enforcement metadata through the F05 lossless JSON codec. Count
and timestamp coercions match the captured source legacy cases. Invalid state,
negative/overflowing counts, malformed collection shapes and duplicate nonempty
attempt IDs are rejected. Legacy empty attempt IDs remain unchanged.

`normalize_legacy_evidence` materializes an implicit completed legacy attempt
before prepared retry/reset or initialization can erase its cursor, query ID,
coverage and enforcement. Empty attempt ID and zero attempted timestamp preserve
the fact that original identity/time is unknown. This is explicit hardening over
the source reset-before-initialization edge; the captured source observations
remain unchanged. The authoritative transition boundary rejects generic callers
that clear this implicit evidence or initialize it away. W02 must call this helper
before applying reset/accounting/reservation to a legacy candidate.

`initialize_budget` only lowers ceilings and infers pre-ledger usage by the source
rules. `reserve_attempt` interrupts an uncertain prior running attempt without
refunding it, checks attempt/engine/result capacity, clears current query fields,
appends a fresh attempt and charges attempt/engine slots. `finish_attempt` settles
once, bounds admitted results, charges every admitted result including duplicate
leads, and deduplicates only newly seen lead identities. All clocks and attempt
identifiers are explicit inputs. Source legacy recovery synthesizes interrupted
attempts before accounting; the oracle preserves that exact reconstruction rule.

`ssx_infra::valkey::research::ResearchStore` uses F07 connections, literal tenant
keys, typed leases and source-exact idempotency/ready/recent assets. Its authority
is Valkey alone. `create` is admission-only; `claim` recovers queued/orphaned work;
`claim_prepared` reapplies retry/extension/metadata mutations to an authoritative
record under a fresh lease. `mutate` reloads current state, merges cancellation,
validates evidence and accounting deltas, and atomically compares both the lease
and original raw record bytes before writing. Stale owners and concurrent writers
sharing one token cannot overwrite each other's history. The validation boundary
also rejects arbitrary mutation closures that append work/admissions without
charges, increase ceilings, refund counters, or add new work above lowered limits.
Already consumed usage can legitimately exceed a newly lowered ceiling.
Fresh attempt IDs must be valid and unique; introducing an empty ID is allowed
only for a faithful historical legacy snapshot. Existing legacy IDs remain
unchanged. New lead evidence must have same-attempt result admissions with
snapshot cursor/index identities and conserved result charges. The global seen
history extension must exactly match unique canonical new lead evidence; it
cannot grow independently or repeat already known IDs. Imported legacy history
remains the preserved baseline for this extension check.

A reservation is safe to dispatch only after `mutate` returns acknowledged
success. `CompletionUnknown` and Lua partial-write errors retain F07 meanings:
reconcile authoritative state before deciding what happened, and never dispatch
or retry automatically. Losing a lease cannot retract an external request.
W01 does not claim exactly-once external execution or implement the W02 runner,
keepalive supervision, service dispatch, or F15 integration.

Cancellation uses a separate durable key. Live owners observe it on reads and
mutations; callbacks cannot erase an already observed side-key cancellation.
Validation uses the merged observation while CAS independently compares original
raw bytes. Unowned cancellation reloads under a fresh lease before finalizing.
A cancellation arriving after the read can still race a commit/dispatch; W02 must
observe cancellation again at its dispatch boundary. This package does not claim
an atomic exclusion guarantee across external dispatch.
Absolute deadline equality expires at claim/admission, and terminal expired jobs
do not resurrect when a supplied wall clock moves backward. Record retention is
the source 86,400 seconds, distinct from job execution deadline. Reads retain
terminal evidence until physical retention expires. Ready/recent indexes are
derived discovery state; bounded SCAN reconciliation and explicit repair rebuild
them without granting ownership. Literal colon-containing tenants are supported;
job IDs containing colons are rejected to avoid tenant/job key aliasing.

## Native evidence

Run with the official disposable Valkey8.1.10 binary built by the accepted F07
script, never an application store URL:

```sh
SLOPSEARX_TEST_VALKEY_SERVER=/path/to/private/valkey-server cargo xtask check W01 --output target/rust-evidence/W01.json
```

The native gate checks independently pinned exporter/corpus digests and exact
source Git blob/current-file identities, then runs 10 source-bound multistep
record/accounting cases, 56 coverage-classification cases, malformed/immutable
record checks, the accepted F07 real-store gate and W01 private-Valkey contracts.
Missing/corrupt evidence, metadata that declares its own new digest, a missing
server executable, or a server version other than 8.1.10 fails. Rust CI retains
W01 evidence alongside existing foundation gates; no existing gate is removed.

Real-store cases cover tenant/ID/idempotency denials, create collisions, competing
same-token writers, stale owner writes/renew/release, reserve/crash/reclaim,
completed evidence preservation, arbitrary-closure accounting bypass rejection,
prepared direct mutation guards, initialization-plus-work scope/implicit-charge denials,
legacy retry evidence materialization, cancellation, exact deadline expiry, restart and
clock rollback, ready/recent index repair, and an actual timed-out reservation
that subsequently commits. Test processes create random private directories and
Unix sockets with TCP disabled. They ignore application URLs, own cleanup, and
never flush a shared store. The failed initial concurrent-test scheduling attempt
was stopped with its identified private child; the corrected test runs outside
runtime worker threads and passes. No provider requests were made.

The developer-only Python3.14.7 exporter is isolated migration evidence. It checks
pinned source bytes before imports, injects source clock/UUID factories, uses
synthetic records, and never contacts a store or provider. Native gates and runtime
never execute it. Re-capture with:

```sh
/private/tmp/slopsearx-rust-oracle-py314/bin/python -B docs/rust-rewrite/w01/oracle/export.py --source-root /clean/pinned/source --output crates/xtask/fixtures/w01/corpus.json
```

Review both artifacts before changing native digest pins. Record and inspect
failed/negative results; test counts alone do not establish distributed safety.

Portal impact: this package does not wire into the active Python server/portal or
change its browser-visible contract. Later shared-runtime integration owns the
portal contract/browser gate. The existing Python portal remains active.

Downstream: W02 receives authoritative record, reservation/settlement, prepared
admission, discovery, cancellation and typed ownership APIs. W01 lead settlement
currently accepts validated `LeadId` values at the application boundary. Actual
URL-to-lead identity wiring awaits acceptance of F11's shared normalization leaf;
injected identity accounting evidence alone is not final W01 acceptance.
