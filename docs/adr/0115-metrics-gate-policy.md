# ADR 0115: Metrics Gate Policy and Release Hook

- Status: accepted
- Date: 2026-11-12
- Related: SA97-07

## Context

`1agent metrics` reported local structural rates but had no threshold
evaluation. The 0.97 release gates M4 (crash recovery), M5 (replay), M13
(audit completeness) and M17 (compacted replay) therefore depended on a human
reading numbers. `release-check.sh` had no hook for them.

## Decision

1. `metrics::MetricsGatePolicy` carries optional minimum bps thresholds for
   crash-free, recovery, replay, compacted replay, tool success and audit
   completeness, plus an optional maximum denied-observation count.
2. `evaluate_gate` fails a minimum gate closed when the sample is missing. A
   threshold of `0` explicitly disables it.
3. `1agent metrics` gains the matching `--require-*-bps` and
   `--max-denied-observations` flags. Values above 10,000 bps are rejected
   before evaluation. Violations are printed to stderr and the command exits
   non-zero; the JSON report shape is unchanged.
4. `scripts/gate-check.sh DIR` requires a journal directory and defaults to
   M4 ≥ 9,500 bps, M5 = 10,000, M13 = 10,000 and M17 = 10,000. Environment
   variables can tighten or disable individual thresholds.
5. `scripts/release-check.sh --metrics-dir DIR` runs the gate check. Without
   the flag it prints that the metrics gate was skipped; it does not silently
   claim a pass.
6. M6 (zero security incidents) is not derivable from journals and remains a
   separate security-suite gate.

## Consequences

- A release check can now fail closed on a real journal directory.
- The gate is only as representative as the supplied journals. A run with no
  resumed run or no compacted run fails the default gate closed, which is
  intentional: absent evidence is not a pass.
- Tool success and crash-free are optional gates because they are targets, not
  0.97 hard gates.
- There is still no hosted CI service; the hook is local and explicit.
