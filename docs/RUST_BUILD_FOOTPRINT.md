# Rust Build Footprint

Use this checklist for local Rust development, qualification, and PR delivery.
It tracks where build material goes and when it can be removed; it does not
set an arbitrary disk-reserve or performance target.

## Before building

- Use `/Volumes/tank01/magnus/rust-builds/slopsearx/<issue-or-pr>/<run>/` for
  the run. Keep Cargo targets, Cargo caches, source copies, overlays, archives,
  logs, raw receipts, handoffs, and the run's `TMPDIR` under that directory.
  Set `TMPDIR` explicitly for tools that create temporary files. Do not create
  new Rust build artifacts in `/private/tmp`, `/tmp`, or `.codex` worktree
  locations.
- Record the owner, issue or PR, each absolute artifact path, artifact class,
  current bytes, and its retain/delete condition in the persistent task ledger
  under `/Volumes/tank01/magnus/rust-builds/slopsearx/storage-ledger/`. Mark
  shared, live, user-owned, or evidence-bearing paths as retained with the
  reason and owner.
- Record `df` availability and `du` usage for the run directory before work.
  Estimate expected growth from a comparable prior build or the inputs and
  toolchain planned for this run; state assumptions when the estimate is rough.
- Reuse a known exclusive target/cache when its source and toolchain match and
  no other process is using it. Avoid duplicate targets and concurrent writers
  to shared build directories. Do not clean or repurpose existing paths merely
  because they look old.

## After each merged PR

Complete these steps as part of PR acceptance:

1. Inspect the task ledger and the PR's receipts/handoffs for every path the
   run created or reused. Check process and open-file use before cleanup.
2. Keep the canonical source and unique raw outputs, qualification receipts,
   archived test binaries, and evidence needed to reproduce or review the
   decision. Move compact, unique retained evidence to its planned Tank
   location when it currently lives in scratch. Do not retain every full
   temporary source archive indefinitely; remove duplicates or superseded
   source copies when their retention condition is met and no receipt or
   handoff needs them. Keep shared targets and other owners' material.
3. Remove only confirmed, owned build-only artifacts or duplicate/superseded
   task copies whose retention condition is satisfied. Never use a broad
   recursive cleanup against an existing user, shared, source, or evidence
   directory. If a Codex-managed worktree is no longer needed, archive it with
   the Codex app tool; inspect ordinary worktree branch and dirty state before
   considering its removal.
4. Measure the cleaned task directory with `du` and check `df` again. Record
   what was removed, logical bytes reduced, available-space change, paths kept
   and why, and any process/reference that prevented cleanup. Filesystem space
   may not increase by the logical `du` reduction (for example, when blocks
   are shared or reclaimed asynchronously); report both measurements without
   claiming unobserved physical reclamation.

If a reference or active use is unclear, retain the path and record the reason
for a later audit. Cleanup never authorizes deleting canonical source, unique
raw evidence, archived test binaries, or another owner's work.
