# Saved-development wire-compatibility method

This separately versioned material set keeps the registered cohorts, scoring
instructions, provider schedule, numerical limits, quality thresholds, and
band gates fixed. Its protocol changes only `candidate_source_sha256` to bind
the bounded parser-wire compatibility fix. That fix validates rounded score
means against feasible two-decimal intervals with unit probability mass and
treats only a one-ULP difference at the maximum as a tie. Reported scores and
ranking instructions remain unchanged.

The companion contract binds the same `D26-R` and `C26-R` four-case groups.
The old saved-development materials remain valid only for their original exact
protocol/cohort pair. Neither this method nor its engineering fixtures rescue
or reuse earlier results. The lock records zero search/capture calls and no
study admission; it is not a stage registration or authorization.

`material-lock.json` records the protocol, cohorts, candidate parser, and
companion digests. The candidate parser digest was computed from the reviewed
source bytes; runtime checks still require the exact source and registration
pins before any stage can proceed.
