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
| Release-profile tests | 127 unit + all E2E passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| remote HTTP readiness rejected | passed |
| local unauthenticated HTTP unchanged | passed |
| credential secrecy | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Release artifacts

Source commit: `bf393a15c4c8bd4f234282bc21ab3d35e8d40b89`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `7764a998583e18a2b801788af3514928c286d108df558cbb80b6358c080cf258` |
| Windows `zip` | `dbdb3dea11dd324487d9441dd4f25124dd25ec5cf35c66f9ca7d07ba6357e2ca` |
| Release manifest | `414d9a4272773e64020674aa7e9dc0b250de280c608a6c05f86178a0821cf85d` |
| Linux binary | `6ff8b82a34fa78420cdf860d122cee7008cd2534115165192c62aed40433a36a` |
| Windows binary | `f618e4111427a163f78d2b702b5d42b4dfb352741e970e2525fde5203e895a9f` |

## Known limitations

- Readiness validates transport and credential presence, not live endpoint availability.
- Placeholder credentials are used only in memory for configuration validation.
- Actual credentials remain environment-owned.

## Packaged binary smoke

`dist/1agent-0.90.0/linux-x86_64/1agent` reported version `0.90.0`, ran the core suite at 6/6,
and inspected its generated report through the compact summary path.

## Next requirement

Return to live-provider and target-host evidence collection.
