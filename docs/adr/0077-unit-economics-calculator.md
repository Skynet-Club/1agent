# ADR 0077: Unit Economics Calculator

- Status: accepted
- Date: 2026-11-12

## Context

SA99-67 asks for subscription + usage-pack + BYOK pricing and a gross-margin
dashboard. Price points are a product decision and should not be invented in
code. The arithmetic, however, is deterministic and worth testing.

## Decision

1. Add `UnitEconomics { revenue_micros, variable_cost_micros,
   fixed_cost_micros }` and `evaluate(economics, outcomes)` in
   `src/unit_economics.rs`.
2. The report contains:
   - contribution = revenue − variable cost, signed;
   - net = contribution − fixed cost, signed;
   - gross-margin bps = contribution / revenue × 10000, `None` at zero revenue;
   - per-outcome revenue and variable cost when outcomes > 0;
   - break-even outcomes = ceil(fixed cost / per-outcome contribution), only
     when the per-outcome contribution is positive.
3. `1agent unit-economics` exposes the calculator with micros inputs and an
   optional `--outcomes`.
4. No prices, plans, included quotas or BYOK rates are built in. A real pricing
   model must be supplied by the caller/product decision.
5. The dashboard is not implemented; this is the calculation layer.

## Consequences

- Gross margin and break-even are reproducible and testable.
- A zero-revenue or loss-making product reports `None`/negative rather than a
  fabricated healthy margin.
- Integer division truncates per-outcome values; cents-level precision is the
  caller's choice of micros.
- Fixed cost is treated as a period cost; allocation rules are out of scope.
- No subscription/usage-pack modeling yet.

## Evidence

```bash
cargo test unit_economics --lib
cargo test --test unit_economics_cli
```

The tests cover positive contribution with break-even rounding, a negative
contribution, zero revenue and zero outcomes, and the CLI JSON fields.
