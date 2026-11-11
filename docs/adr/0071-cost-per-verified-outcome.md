# ADR 0071: Cost per Verified Outcome

- Status: accepted
- Date: 2026-11-12

## Context

The value report shows cost per completed run. A "completed" run can still be
wrong, and SA99-60 asks for cost per *verified* outcome as the cost metric.
`budget::cost_per_verified_outcome` already implements the arithmetic, but the
value report did not expose it.

## Decision

1. Add `verified_outcomes: Option<u32>` and
   `cost_per_verified_outcome_micros: Option<u64>` to `ValueReport`.
2. `build_value_report` accepts the verified count from the caller and uses
   `budget::cost_per_verified_outcome`. A zero or missing count yields `None`,
   never a division by zero or a zero-cost claim.
3. Add `1agent value-report --verified-outcomes N`. The text output prints both
   the count and the unit cost; JSON carries both fields.
4. The report does not infer verification from `completed` or from the
   presence of an answer. The verifier owns that count.

## Consequences

- The headline cost metric can distinguish completed work from verified work.
- A caller that supplies a wrong count gets a wrong metric; the report is
  explicit about the field being supplied.
- Historical reports without the flag now include `null` for both fields.
- No verifier registry or automatic verification-result aggregation exists
  yet; that is a separate integration.

## Evidence

```bash
cargo test value_report --lib
cargo test --test value_report_cli
```

The tests cover cost per completed and per verified outcome, a missing count,
and the CLI flag.
