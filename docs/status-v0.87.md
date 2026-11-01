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
| Clean-source package gate | pending |

## Known limitations

- Text labels are derived from validated summary state and are not independently signed.
- HMAC report authentication remains symmetric.
- Human output remains a status view, not the full case report.

## Next requirement

Return to the next-phase evidence plan; live provider and target-host validation remain the
blocking work.
