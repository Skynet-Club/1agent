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
| Clean-source package gate | pending |

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
