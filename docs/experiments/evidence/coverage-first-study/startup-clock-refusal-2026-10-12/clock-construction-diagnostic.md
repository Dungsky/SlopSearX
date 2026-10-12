# Synthetic clock-construction diagnostic

This note records an offline source reproduction, not a trace of the terminal invocation. The stage limit remains 28,800 seconds; no gate or budget change is proposed.

With synthetic start `s = 60000.123456789`, constructing the deadline as `d = s + 28800` and subtracting `d - s` yields `28800.000000000007`. The unchanged cap check therefore rejects the span as over limit. Moving the synthetic deadline one representable float toward `s` yields `28799.999999999993`, which passes the same check.

The actual terminal refusal did not record its numeric elapsed span, so the synthetic arithmetic demonstrates a reproducible boundary defect in clock construction but does not prove that the exact synthetic values occurred in that run. A safe implementation should avoid constructing an over-cap duration through floating-point rounding while retaining the existing 28,800-second validator and all study gates.
