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
| Clean-source package gate | passed |

## Release artifacts

Source commit: `379a8d6501011ba0a9b301973169fb4683da9105`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `cd25074fefddccd8407c69ce77d9d43175f834da7eead5f0fa72eb1245ddbdb9` |
| Windows `zip` | `ce87e7772128ca258538074b02e19310261719ec8f7f6c054e5969952afdd794` |
| Release manifest | `d28806a33502bbb90558fe1771ac9f69edddeb634eaa0847bdb29822bdd86a85` |
| Linux binary | `bec3505d6c54b5b80b3f34d3c23ea4b67abfa1c4406c1ce9474d2c4521de7d41` |
| Windows binary | `afc051fbac88b1e3b41ce870bfcf2acbd8ea0d200cf0cb40e870ffd777db6582` |

## Known limitations

- This protects transport, not provider-side credential custody.
- TLS trust uses the platform/reqwest trust store.
- Loopback HTTP with credentials remains intentionally available for local development.

## Next requirement

Return to live-provider and target-host evidence collection.
