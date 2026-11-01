# Status v0.74.0

Date: 2026-11-02
Status: suite decision-completeness aggregation implemented

## Changes since v0.73.0

Evaluation reports now include:

```text
decision_completeness {
  cases,
  ok_cases,
  completion_rate_bps,
  conditional_answers,
  conditional_with_bound_flip,
  bound_flip_conditions,
  unbound_flip_conditions,
  high_impact_unknowns_without_reduction,
  completed_with_high_impact_assumptions
}
min_decision_completeness_bps
decision_completeness_floor_passed
```

`model_quality` suites require 100% decision completeness. Contract fixtures and unspecified suites
do not apply the floor.

## Verification

| Check | Result |
|---|---|
| Unit tests | 119 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| aggregate case counts | passed |
| completion rate basis points | passed |
| model-quality floor status | implemented |
| JSON schema | passed |
| human-readable floor line | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- The model-quality floor is fixed at 100%.
- Contract fixtures intentionally do not enforce the floor as model evidence.
- Aggregate rates are unweighted by case difficulty or cost.
- A suite with zero cases remains invalid before aggregation.

## Next requirement

Make the decision-completeness floor suite-configurable:

1. add a bounded basis-point policy field;
2. default model-quality suites to 100%;
3. allow contract fixtures to remain at zero;
4. validate the range;
5. test pass/fail boundaries.
