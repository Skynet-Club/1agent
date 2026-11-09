# Status v0.85.0

Date: 2026-11-02

Status: report approval negative-path hardening implemented

## Added coverage

The signed-report CLI test now verifies:

- an `evaluation_report` approval is accepted with an Active registry key;
- partial approval proof arguments are rejected;
- a correctly signed envelope with `gold_set` artifact kind is rejected;
- an otherwise valid report approval is rejected after its key is revoked.

No runtime behavior changed; this release proves existing failure paths at the executable boundary.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| signed report accepted | passed |
| partial proof rejected | passed |
| wrong artifact kind rejected | passed |
| revoked key rejected | passed |
| unsigned report remains inspectable | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Release artifacts

Source commit: `1afa3770bb28fa3c9c5c0723b76d6de6e5a5329a`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `a25996a7885ecad3d20375e28fb4b314ec96640d88203dc50c812ab1a03b5cc1` |
| Windows `zip` | `33a84b0846516a1c3f347e9ae0dda335b79b1c639fa20d6bcba730a4f1c63d22` |
| Release manifest | `7bf7d729e34a140736bae93d545c463c09ce34bfb71a67a1658086b96bc93d87` |
| Linux binary | `c13decb3a4142453ad418c541083616e33891d0d019bc730ec67ad2adee249bb` |
| Windows binary | `c71f6ea11d12d8f5b59fc16aa89c2d13d8b08585b50fa718c44cc72ddb46605a` |

## Known limitations

- Approval remains symmetric HMAC, not asymmetric signatures.
- Registry state is trusted from the supplied local registry file.
- This suite does not verify a report approval through registry snapshot chains or external anchors.
- Negative tests prove rejection behavior, not key custody in production.

## Next requirement

Evaluate an asymmetric report-signature adapter behind the existing trust-state contract:

1. keep canonical report digest and trust states unchanged;
2. support detached public-key signatures;
3. verify signer identity through a configured trust store;
4. preserve HMAC development mode and mark its shared-key limitation.
