# Status v0.87.0

Date: 2026-11-02
Status: human-readable report trust labels implemented

## Output

Concise report summaries now show:

```text
verification=integrity:checked,binding:bound,signature:present,authenticity:verified
```

Unsigned or unbound reports use `unchecked`, `unbound`, `absent` and `unauthenticated` as
applicable. The machine-readable summary contract is unchanged.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| signed text trust labels | passed |
| machine-readable compatibility | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Release artifacts

Source commit: `b9dd9dd18c6901d07876c6a7554b57ba723b7b28`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `d98caee1664b66e4a4471a6fb393636ba33474f57e15c1e259cff9ecfba772d1` |
| Windows `zip` | `556d234a66d08c4a7b357ec45dd6f361c0c5654d7476fec54cc0283329186d4c` |
| Release manifest | `027d84c706f532b5da7ec30c7480a799a8a2412fbfa8cf94cf9044204427acb3` |
| Linux binary | `b9ca5b129eaf95835fc266a40caf265d7c7df2b38694696220628e5105871d36` |
| Windows binary | `af23ddaa2761752bef058b1db4577ec2fb17e2e35ce7c186103a9aaf951db825` |

## Known limitations

- Text labels are derived from validated summary state and are not independently signed.
- HMAC report authentication remains symmetric.
- Human output remains a status view, not the full case report.

## Next requirement

Return to the next-phase evidence plan; live provider and target-host validation remain the
blocking work.
