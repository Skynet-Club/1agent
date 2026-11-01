# Status v0.83.0

Date: 2026-11-02
Status: host-selectable report trust requirements implemented

## Trust policy

All inspection always validates report integrity. Additional requirements are explicit:

```text
--require-content-binding
--require-authenticity
```

- Default inspection remains informational and may display unbound/unauthenticated reports.
- Content binding requires `content_sha256`.
- Authenticity currently always fails with an explicit unsupported-signature error.
- No mode upgrades an unauthenticated report to authenticated.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| default trust policy | passed |
| required content binding | passed |
| unbound report rejected | passed |
| required authenticity rejected | passed |
| integrity remains mandatory | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- Report signing and trust-anchor verification are not implemented.
- The authenticity option is fail-closed and currently cannot succeed.
- Content binding does not identify the producer.
- Integrity validation does not prove evaluation truth.

## Next requirement

Introduce a signed report envelope with a documented canonical payload and key identifier:

1. sign the report digest;
2. reject modified payloads before signature verification;
3. expose signature presence separately from trust-anchor validation;
4. keep unsigned report inspection available.
