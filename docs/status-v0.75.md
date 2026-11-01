# Status v0.75.0

Date: 2026-11-02
Status: configurable decision-completeness floor implemented

## Policy

Optional suite field:

```json
{
  "min_decision_completeness_bps": 9500
}
```

- Range: `0..=10000`.
- Model-quality default: `10000`.
- Contract-fixture and unspecified default: `0`.
- Explicit values override defaults.

## Verification

| Check | Result |
|---|---|
| Unit tests | 120 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| model-quality default | 10000 bps |
| fixture default | 0 bps |
| custom floor | accepted |
| above 10000 | rejected |
| suite deserialization compatibility | preserved |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Floor is global for a suite, not per case or mode.
- Basis points quantize completion rate to 0.01%.
- Rates are unweighted by difficulty or cost.
- Contract fixtures do not independently enforce model quality.

## Next requirement

Version the decision-completeness policy in evaluation reports:

1. add a policy version;
2. record whether the floor was defaulted or explicit;
3. expose source metadata in JSON;
4. preserve old report decoding;
5. test default, explicit and invalid source states.
