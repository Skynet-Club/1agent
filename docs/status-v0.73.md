# Status v0.73.0

Date: 2026-11-02
Status: decision-completeness scorecards implemented

## Changes since v0.72.0

Each evaluation case now includes:

```text
decision_completeness {
  conditional_answers,
  conditional_with_bound_flip,
  bound_flip_conditions,
  unbound_flip_conditions,
  high_impact_unknowns_without_reduction,
  completed_with_high_impact_assumptions,
  ok
}
```

A decision-completeness failure is a hard gate. Conditional answers require a target-bound flip;
all flips must be bound; high-impact unknowns require reduction methods; completed answers cannot
depend on high-impact assumptions.

## Verification

| Check | Result |
|---|---|
| Unit tests | 119 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| unbound flip counted | passed |
| high-impact unknown gap counted | passed |
| completed high-impact assumption counted | passed |
| valid conditional rollup | passed |
| human-readable decision metrics | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Rollup validates structural completeness, not semantic quality.
- Legacy answer v1 may appear incomplete under current decision metrics.
- Counts are per case and not yet aggregated across the suite.
- No separate minimum bound-flip expectation exists in suite JSON.

## Next requirement

Aggregate decision completeness across evaluation suites:

1. add suite-level decision totals;
2. enforce a model-quality minimum completion rate;
3. expose rates in JSON and text reports;
4. preserve contract-fixture semantics;
5. test mixed valid and incomplete cases.
