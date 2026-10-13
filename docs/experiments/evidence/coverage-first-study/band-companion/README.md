# Separate research-size qualification companion

This companion adds a stricter delivery condition to the frozen eight-task
mixed-workload gates. It does not change the protocol, cohorts, budgets,
thresholds, sampling plan, or stage schedule. Each registered run must pin the
same shared stage contract, source-closure v2, and initial registration before
admission. The shared contract declares both development and confirmation
membership so the qualified source-closure digest remains identical across the
required development-to-confirmation lineage. The operator still validates
that the actual run is one of those two stage kinds and fails closed if any
binding is absent or does not match.

The gate calculator accepts the original frozen execution-contract digest and
one additional exact digest for the registered acquisition-v2 protocol. That
exception is bound to the pinned registered protocol and cohort hashes and the
`coverage-acquisition-profile/2` schema; acquisition budgets and thresholds
remain the values in those pinned files.

For each exact four-task band, the existing A/B ranking calculation and R1/R2
task-use calculation run again over the same closed grades. The same thresholds
apply independently. The original mixed-workload result remains reported as-is;
confirmation eligibility additionally requires the mixed result and both band
results to pass, together with all original shared navigation, stability,
resource, and terminal gates. Missing evidence is inconclusive. No task can be
dropped, moved, or reassigned to rescue a band.

These are four fixed cases per band, not random samples. Their results support
claims only about these frozen cases and conditions. They do not establish a
population estimate, a causal effect of research depth, or an intent-specific
effect (each band has only two cases of each intent). The companion reuses
existing observations and adds no acquisition, capture, selector, answerer, or
grader calls.
