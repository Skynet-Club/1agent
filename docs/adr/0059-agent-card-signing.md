# ADR 0059: Agent Card Signing

- Status: accepted
- Date: 2026-11-12

## Context

Agent Card v0 is a public statement of capability and limits, but it was
unsigned. A peer cannot tell whether a card was modified in transit or forged.
A2A-style discovery needs a verifiable card identity before it is safe to trust
a remote agent's claims.

## Decision

1. Add an optional `CardSignature { algorithm, public_key, signature }` to
   `AgentCard`. An unsigned card omits the field.
2. The signing payload is the canonical JSON of the card with `signature` set
   to `None`. Serialization is deterministic because the struct field order is
   fixed.
3. `sign_card` accepts an Ed25519 PKCS#8 private key in hex and embeds the
   matching public key and signature.
4. `verify_card(card, trusted_public_key_hex)` fails closed when:
   - the card is unsigned;
   - the algorithm is not `ed25519`;
   - the embedded public key differs from the trusted key;
   - the signature is not 64 bytes or does not verify.
5. Add `1agent agent-card --generate-key [--private-out PATH --public-out
   PATH]`, `--sign-key-file <PATH>`, and `--verify <PATH> --trusted-key <HEX>`.
   The file path is preferred: the private key is written as a new 0600 file
   and is not printed. `--sign-key <PKCS8_HEX>` remains for compatibility but
   exposes the key in argv. A failed verification exits non-zero.
6. Add `signed_agent_card` as a verified capability in the card.
7. This is a local signature and trust anchor. It is not A2A registry
   discovery, key distribution, revocation, or a signed registry entry.

## Consequences

- A card can be shipped with a stable public key and checked by a peer.
- Verification is offline and does not depend on a model or network.
- The same trust-anchor problem remains: the verifier must obtain the public
  key out of band. Key rotation, revocation and registry discovery are
  separate slices.
- Signing does not make capability claims true; it only binds the claims to a
  key.

## Evidence

```bash
cargo test agent_card --lib
cargo test --test agent_card_cli
```

The tests cover round-trip verification, tampering, a wrong trusted key, an
unsigned card, CLI key generation, sign-then-verify, and a tampered card that
must fail.
