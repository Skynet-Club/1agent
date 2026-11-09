# Status v0.76.0

Date: 2026-11-02
Status: versioned decision-completeness policy metadata implemented

## Report contract

Evaluation reports now include:

```json
{
  "decision_completeness_policy": {
    "version": 1,
    "floor_source": "default"
  }
}
```

- Generated reports use decision-completeness policy version `1`.
- `floor_source` is `explicit` when the suite sets `min_decision_completeness_bps`.
- `floor_source` is `default` when the suite relies on a kind-aware default.
- Reports without the metadata decode as version `0` and `legacy`.
- Unknown sources and unknown object fields fail decoding.
- Human-readable evaluation output includes policy version and floor source.

## Verification

| Check | Result |
|---|---|
| Unit tests | 121 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| default source metadata | passed |
| explicit source metadata | passed |
| legacy report decoding | passed |
| invalid source rejection | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Release artifacts

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `90223e088d270d433617e7fe0dfdd0bae2cafb8f09c416b9c77e158983bedd86` |
| Windows `zip` | `88328431a575f47eaf21b7f1b3a86cdd3bd3cb313da08be9fa273fa59aabb6b5` |
| Release manifest | `5d4ef40660f0e3f2c0eb04683ff8bb5060189f660117819895cab466c7551c90` |
| Linux binary | `804a54a1ccb9333ef28c406a3e48ac863fab45f62d976e35150b0796348fa644` |
| Windows binary | `350ea072e3768815a7217565a045a7e29c44d8883923f117732447aa775d6add` |

## Known limitations

- Policy version metadata records the interpretation rule; it does not prove case semantics.
- Legacy reports are labeled after decoding but cannot recover their original provenance.
- The floor remains suite-global and unweighted.
- Report metadata is not cryptographically signed.

## Next requirement

Expose decision-completeness scorecards through a stable machine-readable evaluation
inspection path for externally generated reports:

1. validate report policy metadata against supported policy versions;
2. fail closed on internally inconsistent policy/source combinations;
3. preserve legacy version `0` reports;
4. test valid current, valid legacy and invalid combinations.
