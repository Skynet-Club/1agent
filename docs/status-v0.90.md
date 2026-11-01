# Status v0.90.0

Date: 2026-11-02
Status: provider readiness transport consistency implemented

## Change

Provider readiness now constructs the OpenAI-compatible adapter with a placeholder credential
whenever `api_key_env` is declared. This makes `providers --check` apply the same HTTPS rule as
actual provider construction.

A remote HTTP endpoint with a declared credential now reports a configuration error instead of
appearing ready.

## Verification

| Check | Result |
|---|---|
| Unit tests | 127 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| remote HTTP readiness rejected | passed |
| local unauthenticated HTTP unchanged | passed |
| credential secrecy | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- Readiness validates transport and credential presence, not live endpoint availability.
- Placeholder credentials are used only in memory for configuration validation.
- Actual credentials remain environment-owned.

## Next requirement

Return to live-provider and target-host evidence collection.
