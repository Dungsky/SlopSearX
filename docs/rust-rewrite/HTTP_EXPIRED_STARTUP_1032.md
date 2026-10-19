# HTTP expired startup admission (#1032)

Source proposal only. Native tests and current-candidate Linux AMD64/ARM64
qualification have not run for this successor. This work follows the
[Rust rewrite case study](CASE_STUDY.md) and
[issue #1032](https://github.com/magnus919/SlopSearX/issues/1032).

## Observed failure and diagnosis boundary

[PR #1024 ARM job 118105702415](https://github.com/magnus919/SlopSearX/actions/runs/39266027319/job/118105702415)
at exact head `84b2c39f76dc5a426f4f455b67ded10be3753e65` failed the native
biomedical step with 262 passed and one failed. The failing control,
`http::a10_owner_clock_tests::unmocked_system_expiry_and_expired_search_context_dispatch_nothing`,
received `Error::Deadline` but found a retained client at the assertion
`expired unmocked HTTP never checks out/builds a client`. This is a failed
assertion, not a wall-budget timeout. The original source and its 250us past
deadline remain unchanged; SHA-256 of `tests/support/a10_http_clock.rs` is
`b850833e2cac860cce876858fdff32130640b07fd2192f4f74b34f8729676c72`.

At baseline `24ee06a2a7500892315ad38afbe1d72952f4caec`, the production HTTP
owner reads `std::time::Instant`, but its sleep delegates to Tokio's timer.
`Session::execute` checked stop and cancellation, then relied on a biased
select's sleep branch before polling `execute_inner`. If that sleep is still
pending after the owner deadline, the inner future can spend request budget
and check out a client. The original failure proves the client-side effect;
it does not expose the precise timer-wheel state on that ARM poll.

The proposed guard compares the same owner clock directly to the unchanged
caller deadline after stop/cancellation checks and before header work or the
select. The select continues to own in-flight expiry. No deadlines, timeout
outcomes, source observations, adapter inventories or suite concurrency are
changed.

## Focused controls and acceptance plan

The additive `tests/support/http_expired_startup.rs` module contains four
controls, appended by exact name to the current compiled A13 inventory:

- System expiry with Tokio deliberately paused: real time crosses a 250us
  deadline, the timer is first-polled and must still be pending, and execute
  must immediately return Deadline with zero request accounting, transport
  calls or clients. Both an injected transport and the real pool are covered.
  All connection permits are held to prevent any socket even if the guard
  regresses. Pausing belongs only to this test's isolated Tokio runtime.
- Manual expiry exactly at 250us and at 251us, with zero side effects.
- Closed before Cancelled before Deadline precedence with expired startup.
- Successful System dispatch and Manual dispatch at 249us of a 250us owner
  budget, despite expired wall time. This rejects a guard using System time
  in place of the Manual owner clock.

After a root grant for the existing shared Tank target/cache, serialize native
execution with other rewrite work. Keep commands offline/locked; use bounded
Tank logs and temp paths, preserve every failure, and measure target/cache
before and after. First compile/run the four controls and the unchanged
original failure control. Run the System causal test against a temporary
source mutation removing only the new guard: it must fail at the first-poll
Deadline assertion, with connection permits still preventing socket dispatch;
restore and hash-verify the exact candidate before the positive rerun. This
mutation demonstrates the admission gap, not the undocumented timer state of
the original ARM failure. No Python backend/oracle/provider runs are needed.

Then run the complete native `ssx-infra --lib` suite, the source-amendment
validator's positive/mutant controls, and native A07/A10/A11/A12/A13 gates.
Retain the original full platform and adapter gates in current-candidate
Linux AMD64 and ARM64 CI. A focused pass alone does not close #1032 or unblock
#936/#897. The separate #1030 CI wall-budget change is not evidence for this
behavioral fix.

## Source bindings and portal impact

The V6 source amendment preserves V5/V4/V3/V2 and all historical source
containers. Only the HTTP successor advances; the exact baseline HTTP source
is retained in `a13/source-amendments/http.rs.24ee.frozen`. The new control
module has an exact path/bytes/mode/SHA binding and negative checks against
modified source and substituted paths. No original provider capture or frozen
qualification JSON is repinned. Four new names augment the current compiled
inventory; all historical names remain required.

Portal impact review: this fixes native adapter HTTP admission, so expired
work stops before network resources are used. The accepted timeout result,
portal request state, result fields, access policy, filters, cache/snapshots,
and HTML/API error shape are unchanged. The original expired search-context
control still requires the timeout outcome and empty results. No browser
contract amendment is needed; current-candidate CI portal contracts remain
required, alongside native timeout controls. Python backend execution remains
outside this source-only phase.
