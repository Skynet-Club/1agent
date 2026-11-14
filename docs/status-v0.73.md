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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Rollup validates structural completeness, not semantic quality.
- Legacy answer v1 may appear incomplete under current decision metrics.
- Counts are per case and not yet aggregated across the suite.
- No separate minimum bound-flip expectation exists in suite JSON.

## Verified clean release

```text
source commit: 031af0284a1453ccb09c67986831d89bb03141c8
git dirty: false

Linux binary:
c2636c86256c3112c37e06b9f3a6ef775f7ce72efaadc1e0c405375ad370ad8d

Windows binary:
ba04f8e94716108924951dd0b41838e6f1c894975295280441cd3cc1e3c961bf

Linux tar.gz:
5ef0c0834f2f027fa2fc0e4ee040c01249cde7f425670c1a5e97bbb6ce219c4e

Windows zip:
6619d96b3983c8204ce1888a107dbe3c2f4f382c6412c51184d5d40a76afdfd7

Release manifest:
9cd6819e30d04eee6c02f667a5a8a1d771e75e25b536a192864d1dfb3f255836
```

## Next requirement

Aggregate decision completeness across evaluation suites:

1. add suite-level decision totals;
2. enforce a model-quality minimum completion rate;
3. expose rates in JSON and text reports;
4. preserve contract-fixture semantics;
5. test mixed valid and incomplete cases.
