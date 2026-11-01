# Status v0.86.0

Date: 2026-11-02

Status: report verification boundary hardening implemented

## Fixes

- Partial approval proof arguments are rejected before the report path is read.
- Summary `passed + failed` and report count checks use `checked_add`.
- Maliciously large summary counts fail validation rather than risking debug overflow.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| partial proof before file access | passed |
| count overflow boundary | passed |
| signed report still accepted | passed |
| unsigned inspection compatibility | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | pending |

## Known limitations

- This is a boundary-hardening release, not evidence of real provider quality.
- Counts are still deserialized as platform-width `usize`.
- HMAC report authentication remains symmetric.

## Next requirement

Continue the environment-evidence plan; do not expand autonomy until live provider and target-host
gates are available.
