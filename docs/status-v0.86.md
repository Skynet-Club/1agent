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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `c2923668883bdd0ec5c722fc2eb700471339f712`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `89b8c7a4fa25e2aeeab588d8674b7124c277aec0536704a8477e5d9b18e4a622` |
| Windows `zip` | `e26b3577fc95c1c89ca23b55e6076dabfb67a17b3a5384124fdc00adc7d50a68` |
| Release manifest | `c4c7cc9f56f7297728cab0058f9955bbea2743c9f616823489a52b34aead83eb` |
| Linux binary | `4d227c5055207d3b61a41376c402c4d002a964e259aeb17ec1c4874d7da46493` |
| Windows binary | `6a9d1cbbb4e2c1f2906d7f585ba8f06fd51366bfdbe6dbaac152451737eb11e1` |

## Known limitations

- This is a boundary-hardening release, not evidence of real provider quality.
- Counts are still deserialized as platform-width `usize`.
- HMAC report authentication remains symmetric.

## Next requirement

Continue the environment-evidence plan; do not expand autonomy until live provider and target-host
gates are available.
