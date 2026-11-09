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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `db5acd62fffeb236ba6b16b2ac496a4a65b19253`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `53539f6d9868e202c32745333aeaf96ff7e24f050ef3a2a87979a97d20c2c858` |
| Windows `zip` | `61ff8d01c2d47675ddc000323d652b387b9dc03e0528347ef56044e3d69a6799` |
| Release manifest | `bb5d6205eb865102431d2b9ed003bfb30f5499686241033810b8c4167ecdcb1f` |
| Linux binary | `6a6cff2dbf2eb536fc4ff716c541dc487803e1a5536f9a0443301179cc1a6c55` |
| Windows binary | `b63032a8a53546938c0bcbe0c475d73bf57cefdde26fd558de0ae427a756cb92` |

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
