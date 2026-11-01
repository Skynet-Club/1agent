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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `a90139841005a45fd0113ff806f22bd754615acd`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `4c73a9784dda21ace6df39a2e3a649796a7f91c87841d4a06e9d1b887ece23ba` |
| Windows `zip` | `bbcb53f93d7e83a1f4a22ed26ce0d07bf95bea915020b494c732be361f67aa1e` |
| Release manifest | `85e5fe856817598bd9b13ba008de4fcbbe4b10e8ac0985dd33144defcf4b360d` |
| Linux binary | `d78d83853ba1ce167805c17f4dbc995f6e5c083f16417d68b6816c7f7c402551` |
| Windows binary | `2cdead0db076ec222b91906f0d28b3df5fec69d7dc60c4271f30e5c3117cc749` |

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

Partial proof is rejected by the current CLI path and covered by signed-report E2E.
The report trust boundary is documented in
[`architecture.md`](architecture.md),
[`adr/0004-evaluation-report-trust.md`](adr/0004-evaluation-report-trust.md) and
[`audit-v0.84.md`](audit-v0.84.md).
