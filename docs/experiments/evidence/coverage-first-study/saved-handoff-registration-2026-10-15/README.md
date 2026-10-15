# Saved-corpus handoff registration

Status: full static preflight passed for both stages; neither stage was admitted or invoked; no qualification result.

This public snapshot binds the development and confirmation registrations to source revision `ebeb9cee521612e6a868730812b154933d4863f2`, the registered protocol and cohorts, the mandatory four-per-band companion contract, and source-closure digest `0ffd4f8fea03e6503cc6d6a5e66b78929553fb8a5a334673035bd6142ea471ca`. The four new stage and packet identities are recorded in `identity-allocation.json`; all are distinct from the 95 retired identities.

Development is registered to use the previously saved search results and page captures with fresh blind grades. Confirmation is registered for separate cases and fresh collection/capture, conditional on a passing development receipt. Existing task definitions, bands, thresholds, and resource limits remain pinned by the protocol and cohort files.

The pinned source revision is now merged on `main` at `db5d8dac555683aa9315816fb18d97e39064b86e`, and its required CI checks passed. The initial builder snapshot records that full static preflight had not yet been run. Afterward, the exact registered source revision passed both runner and independent issuer checks for development and confirmation; see [full-static-preflight.json](full-static-preflight.json). These four no-call checks did not admit either stage or issue phase permits. No provider, search, capture, selector, answerer, assessor, or grader calls were made under this registration. There are no late-phase inputs, receipts, blind assignments, selector maps, answer manifests, or assessment results in this snapshot. It does not establish candidate quality or authorize execution. Development has not started. Confirmation still requires a valid passing development receipt.

`static-registration-status.json` and `setup-checks.json` preserve the initial builder snapshot, before the four checks ran. The later receipt records the checks. `public-file-inventory.json` hashes every other JSON and text file in this folder; the inventory file does not hash itself. Late-phase files are placeholders, not completed artifacts.
