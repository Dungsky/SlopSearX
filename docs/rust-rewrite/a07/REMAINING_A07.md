# Remaining work to close A07 / issue926

The runtime/repr/resource freeze is a reviewable component, not issue completion.
The following work belongs to A07 itself and remains required after that review.
Actual five-engine factories and the native gate now pass candidate tests; catalog declarations remain historical; candidate CI gate/artifact wiring and additive inventory have been implemented and await review.

1. Review the implemented biomedical factory registration in
   crates/ssx-infra/src/adapters/mod.rs, following EngineFactory ownership and the
   accepted A01/A04 pattern. Capture the caller's shared HTTP/admission handles;
   construct the five actual adapters with source-compatible resolved settings.
   Add actual registry.instantiate-to-search tests for each engine, with ordered
   requests, errors, cancellation and context, rather than constructor availability
   alone. Duplicate/unknown factory behavior must remain fail-closed.
2. Review the implemented real F06-to-factory config boundary for all five engines. Keep raw
   ConfigScalar bool/integer/float/text values. Preserve Europe PMC disabled by
   default and test opt-in through actual selection/dispatch. The source service's
   _normalize_engine_configs omits only absent/blank base_url values before
   adapter discovery (slopsearx/service.py251-268); missing configs use adapter
   endpoints, and nonblank overrides pass through unchanged. EngineConfig defaults
   do not create entries for most of these providers. The implemented factory bridge applies this normalization and preserves raw settings; seven actual factory/config/scope tests pass.
   This is A07-owned integration, not a downstream excuse.
3. Review the implemented real fail-closed A07 xtask gate and dispatch entry in
   crates/xtask/src/a07.rs and crates/xtask/src/main.rs. It must validate immutable
   nonexecutable source/case/coverage/diagnostic identities, unique complete172
   primary controls and separate auxiliary controls, then execute the actual
   adapter/factory/context/resource/compact native regressions. Missing fixtures,
   changed identities, missing mapped consumers or failed test commands must
   return nonzero. A passing placeholder/count-only collector is insufficient.
   Temporary historical Python oracle files must not become runtime or mandatory
   gate prerequisites: they are inventoried for G08 retirement, not executed or
   required by native replay.
4. Propose and review the shared implementation-status projection consistently
   with the accepted registry/factory contract. Current F06 catalog fragments and
   crates/xtask/src/f06.rs deliberately require all historical descriptors to say
   unavailable, while Registry::is_available reflects attached factories. Do not
   blindly flip the five immutable source projections or claim factory activation
   solely from metadata. Reconcile this with the registry/F06 owner before edits.
5. Review the implemented A07 gate/evidence artifact in .github/workflows/rust.yml
   and the applicable O06/evidence manifests; register the explicit Python
   inventory proposal, including the two new temporary exporters. Complete the
   source-bound coverage disposition for every original test without claiming
   downstream consumer proofs from adapter success tests. Run required native
   checks on the final exact union/head and deliver the reviewed PR/merge workflow
   under the parent owner's publication authorization.

The proposed factory/gate/projection changes above require parent review before
editing their shared paths, as requested at assignment. A07 must not be closed
while required review/CI/inventory/delivery remains open. Factory and gate execution evidence is in FACTORY_GATE.md; presence alone does not complete issue926.

## Genuine shared/downstream boundaries

- Parent review/acceptance of this new cooperative repr/scalar/casefold component
  is required before counting it as qualified shared runtime behavior. Current
  tests are candidate evidence until that review; the payload/batch donor was
  already separately accepted and copied under exact guards.
- Input DOM/F08 storage has its inherited fixed codec/transport bounds, not the
  row ledger. The accepted R2 owned parser depends on F15 absent from this branch;
  guarded parser/F15 reconciliation is a shared integration decision. No wholesale
  parser import occurred. Global admission, retained responses/clones/cache/flight
  lifetimes and cold Unicode-table initialization remain G02 work. The32MiB
  row-output cap does not close those full-milestone requirements.
- T03/G01 owns actual HTML/portal output. Native actual adapter-to-rank-to-compact
  tests now cover the source PPR notice and attribution plus PubMed-first distinct
  URLs. They do not prove HTML rendering, same-URL attribution replacement or
  full MCP transport/policy wiring. T06/transport owners retain the latter boundary.
- Live provider certification and deployment remain unperformed and unauthorized;
  all candidate evidence is offline fixtures, owned loopback or isolated source
  controls. No paid/scientific/provider calls were made.

These shared boundaries remain mandatory for the full milestone, independently
of the A07-owned factory/gate/delivery steps above.
