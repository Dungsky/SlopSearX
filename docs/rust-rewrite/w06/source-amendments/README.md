# W06 source amendment

`current-source-amendment-v1.json` records one narrow successor to the frozen
W06 owned-source pin for `crates/ssx-application/src/lib.rs`. The archived
`de04` bytes are the exact source pinned by W06 qualification before W07. The
only permitted suffix adds the two W07 module exports for `lineage` and
`receipts`. The xtask W06 check validates the manifest identity, archive bytes,
parent Git blob, and complete successor bytes. It rejects any other source
change.

The original W06 source captures, packets, qualification, and their source
bindings remain unchanged. This amendment only reconciles a source identity;
it does not claim W06 behavioral parity or broaden W06 coverage.
