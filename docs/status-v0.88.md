# Status v0.88.0

Date: 2026-11-02
Status: human-readable floor status implemented

## Output

Concise summaries now include a direct floor decision:

```text
floor=9500 bps:passed
floor=9500 bps:failed
```

The machine-readable `min_decision_completeness_bps` and
`decision_completeness_floor_passed` fields are unchanged.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| floor pass text output | passed |
| signed trust text output | passed |
| JSON compatibility | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Release artifacts

Source commit: `c757d5ee456e8ad26c452afd78274d1554b9be4b`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `440898bd92f4850df3ef5bbcfd1a2e765708a5e1925b0a500af830ee51a9906e` |
| Windows `zip` | `e3f6c81d2de2c5e627a9ba898944265fbd9b17a242d7d6917a2596484d9efc30` |
| Release manifest | `0a43ed2c9d4c1558398de6ed4c74af510e47fb0daf5b8616624755a46852ade2` |
| Linux binary | `c15c0abc1e8786551403196b56a40f2c8d966018cf86e34c567b12842beef812` |
| Windows binary | `b2f3acc19ac1d22199c66f0a42bf0cae0f333fb2f33f8fff022def555b3f4bf6` |

## Known limitations

- This is a presentation improvement only.
- The floor remains suite-global and unweighted.
- Human text remains an inspection view, not a signed attestation.

## Next requirement

Return to live-provider and target-host evidence collection.
