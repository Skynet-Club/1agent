# Status v0.89.0

Date: 2026-11-02
Status: provider credential transport hardened

## Security rule

- Remote bearer credentials require HTTPS.
- Plaintext HTTP remains available for loopback local providers without credentials.
- Loopback HTTP with a credential remains allowed for explicitly local development.
- Empty API keys are rejected.
- User/password components embedded in provider URLs are rejected.

## Verification

| Check | Result |
|---|---|
| Unit tests | 126 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| remote HTTP credential rejected | passed |
| loopback HTTP allowed | passed |
| HTTPS credential allowed | passed |
| empty key rejected | passed |
| embedded URL credential rejected | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- This protects transport, not provider-side credential custody.
- TLS trust uses the platform/reqwest trust store.
- Loopback HTTP with credentials remains intentionally available for local development.

## Next requirement

Return to live-provider and target-host evidence collection.
