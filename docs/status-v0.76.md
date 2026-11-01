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
| Clean-source package gate | pending |

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
