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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- The model-quality floor is fixed at 100%.
- Contract fixtures intentionally do not enforce the floor as model evidence.
- Aggregate rates are unweighted by case difficulty or cost.
- A suite with zero cases remains invalid before aggregation.

## Verified clean release

```text
source commit: 67a5f370cbfccaf8d972cb1bc8cb0e491f7ee866
git dirty: false

Linux binary:
b3a32c858f134404d39293af28f87bec74252839a8139d44313ece2e11003932

Windows binary:
dc6c47e7907c58059c89d8ad6742a568bdfe19a8c8260ef6885ba660ec80b8f9

Linux tar.gz:
d90b7967104f3255adff1d0e469236a5523a38fddeae55baf69e05e5889a55b6

Windows zip:
a961c0338f542f04b87128e5c6e7470258830640c1afae1c208aea79622f777d

Release manifest:
010b18d6cebc98621e46647351205edd527fae7b762689b57de54a25f8e07014
```

## Next requirement

Make the decision-completeness floor suite-configurable:

1. add a bounded basis-point policy field;
2. default model-quality suites to 100%;
3. allow contract fixtures to remain at zero;
4. validate the range;
5. test pass/fail boundaries.
