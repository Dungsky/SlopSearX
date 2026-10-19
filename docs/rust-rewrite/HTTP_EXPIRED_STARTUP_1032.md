# HTTP expired startup admission (#1032)

The locally qualified Rust candidate is committed as `c253e604` and normally
merged with the fetched `origin/main` at `ff3a6ede` in merge commit
`ba710dcc`. Root acceptance for the bounded causal and restoration run is
recorded at
`/Volumes/tank01/magnus/rust-builds/slopsearx/storage-ledger/pr1034-wikipedia-reset-causal-root-acceptance-v1.json`
(SHA-256 `c4688030c4d84c089fbee98391edd3869ecf54e43ff1a891efa867c97da3260f`).
This is local macOS qualification evidence. Current-candidate Linux AMD64,
Linux ARM64, and portal CI remain pending; this evidence does not close #1032
or unblock dependent work.

## Observed failure and diagnosis boundary

[PR #1024 ARM job 118105702415](https://github.com/magnus919/SlopSearX/actions/runs/39266027319/job/118105702415)
at exact historical head `84b2c39f76dc5a426f4f455b67ded10be3753e65`
failed the native biomedical step with 262 passed and one failed. The failing
control,
`http::a10_owner_clock_tests::unmocked_system_expiry_and_expired_search_context_dispatch_nothing`,
received `Error::Deadline` but found a retained client at the assertion
`expired unmocked HTTP never checks out/builds a client`. This is a failed
assertion, not a wall-budget timeout. The SHA-256 of
`tests/support/a10_http_clock.rs` at that historical failure head was
`b850833e2cac860cce876858fdff32130640b07fd2192f4f74b34f8729676c72`; that is
a historical hash, not the current file hash.

At baseline `24ee06a2a7500892315ad38afbe1d72952f4caec`, the production HTTP
owner reads `std::time::Instant`, but its sleep delegates to Tokio's timer.
`Session::execute` checked stop and cancellation, then relied on a biased
select's sleep branch before polling `execute_inner`. If that sleep is still
pending after the owner deadline, the inner future can spend request budget
and check out a client. The original failure proves the client-side effect;
it does not expose the precise timer-wheel state on that ARM poll.

The candidate guard compares the same owner clock directly to the unchanged
caller deadline after stop/cancellation checks and before header work or the
select. The select continues to own in-flight expiry. No timeout was
increased, no public request/response contract changed, and no performance
claim is made.

## Local causal and restoration evidence

The candidate adds four startup controls in
`crates/ssx-infra/tests/support/http_expired_startup.rs`, registered in the
existing HTTP test module:

- System expiry with Tokio deliberately paused: real time crosses a 250us
deadline, the timer is first-polled and must still be pending, and execute
must immediately return Deadline with zero request accounting, transport
calls or clients. Both an injected transport and the real pool are covered.
All connection permits are held to prevent any socket even if the guard
regresses. Pausing belongs only to this test's isolated Tokio runtime.
- Manual expiry exactly at 250us and at 251us, with zero side effects.
- Closed before Cancelled before Deadline precedence with expired startup.
- Successful System dispatch and Manual dispatch at 249us of a 250us owner
budget, despite expired wall time. This rejects a guard using System time in
place of the Manual owner clock.

The root-approved one-line mutant removed the second-stage
`session.set_deadline(...)` call in Wikipedia's two-stage search flow. The
selected A04 control then failed with exit 101 and the expected sole panic at
`crates/ssx-infra/src/../tests/support/a04_broad_api.rs:1275:5`:
`second actual stored deadline was not renewed`. Strict restoration returned
the same source inode to its original bytes. This demonstrates that the second
stage uses a renewed operation deadline; it does not replay the original ARM
startup failure or identify its timer-wheel state. A fresh compile and
complete 291-name library inventory check then passed. The 291 names are an
inventory listing, not evidence that all 291 tests ran.

After restoration, all 35 distinct HTTP controls passed: 23 A04 tests, two
endpoint controls, four startup controls, and six A10 controls. The selected
Wikipedia deadline control also passed when run separately; it is one of the
23 A04 tests, so the run had 36 test executions across 35 distinct names.
The earlier source-pinned positive batch also accepted its 112 xtask checks,
63 A13 checks, typed control bridge, and A13 compiled-inventory parser
controls. These counts describe bounded native qualification, not full
workspace or platform CI.

The unchanged 250us System/Manual startup controls remained in the candidate.
The System decoder test stages a 60-second post-exchange wait; that case is
separate from, and is not evidence for, the 250us startup boundary.

## Source bindings and delivery limits

The V6 source amendment preserves V5/V4/V3/V2 and all historical source
containers. The exact baseline HTTP source is retained in
`docs/rust-rewrite/a13/source-amendments/http.rs.24ee.frozen`. The new control
module has an exact path/bytes/mode/SHA binding and negative checks against
modified source and substituted paths. No original provider capture or frozen
qualification JSON was repinned. The four added test names are in the full
compiled inventory; historical names remain required.

The 23-test A04 unit was moved under `tests/support/` without pruning test
names or capturing existing names. The preserved source amendment records
the relocation and exact source bytes. Typed control bindings distinguish
current source from historical source. `SystemOperationTrace` and its
recording hook are guarded by `#[cfg(test)]`; they provide deterministic test
observations and are omitted from production builds.

Portal impact review: this changes native HTTP admission, so expired work
stops before network resources are used. The accepted timeout result, portal
request state, result fields, access policy, filters, cache/snapshots, and
HTML/API error shape are unchanged. Current-candidate portal contract CI is
still required, alongside the pending Linux AMD64 and ARM64 native gates.
Python backend execution remains outside this Rust qualification.
