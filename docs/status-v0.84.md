# Status v0.84.0

Date: 2026-11-02
Status: authenticated evaluation report inspection implemented

## Report authenticity

`eval-inspect` accepts an existing approval envelope plus a key environment name and approval
key registry:

```text
--approval report.approval.json
--approval-key-env REPORT_APPROVAL_KEY
--approval-registry approval-registry.json
```

Verification requires:

- artifact kind `evaluation_report`;
- exact report-byte digest match;
- a valid HMAC-SHA256 envelope;
- an Active or Retired key present in the approval registry;
- a non-revoked key;
- at least 16 key bytes.

Summaries distinguish `signature_present` from `authenticity=verified`. Unsigned reports remain
`unauthenticated`, and `--require-authenticity` fails without a verified proof.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| signed report accepted | passed |
| authenticity requirement satisfied | passed |
| signature presence exposed | passed |
| unsigned authenticity rejected | passed |
| missing proof fields rejected | passed |
| key registry enforcement | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- Current report authentication uses symmetric HMAC, not public-key signatures.
- Any holder of the shared key can authenticate a report.
- The report approval is separate from the release-manifest approval chain.
- Verification proves key possession and byte integrity, not evaluation truth.

## Next requirement

Require complete approval proof arguments as a CLI group and reject partial proof before any
report inspection:

1. enforce all-or-none proof arguments;
2. reject a wrong artifact kind explicitly;
3. ensure revoked keys fail;
4. preserve unsigned informational inspection.
