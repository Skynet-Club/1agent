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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Release artifacts

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `c36df5f1a4bea0a3d5593b7e5c59ab4f79c8c8fd26072ef6f342c2dc462baae4` |
| Windows `zip` | `952d318ede2d191c6a75371d29d0e6b17bb448917da537e753dab01d0ac0b548` |
| Release manifest | `598f970f5b8dfc1cb778b66a489b601a4152c257bee17371309b28f5d2cf02f5` |
| Linux binary | `d01ae68352d441690990ff4c15f2593ecb4b543e3afa4c18aa02a1f8ede5a432` |
| Windows binary | `80ca63ad8d9bc4b68c64d920e42f3ab666a14d7462fce39ea540f9ebc1ac182e` |

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
