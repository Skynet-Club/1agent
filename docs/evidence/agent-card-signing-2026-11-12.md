# Agent Card Signing — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `agent_card::sign_card`, `agent_card::verify_card`,
`agent_card::generate_signing_key` and the `1agent agent-card` signing flags.

## Commands

```bash
cargo test agent_card --lib
cargo test --test agent_card_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 6 tests
test agent_card::tests::card_has_required_fields_and_limits ... ok
test agent_card::tests::card_round_trips_through_json ... ok
test agent_card::tests::signed_card_verifies_with_the_trusted_key ... ok
test agent_card::tests::tampering_and_wrong_keys_are_rejected ... ok
test agent_card::tests::unsupported_claims_are_not_marked_verified ... ok

test result: ok. 6 passed; 0 failed

running 3 tests (tests/agent_card_cli.rs)
test agent_card_cli_reports_capabilities_and_limits ... ok
test agent_card_cli_human_output_lists_limits ... ok
test agent_card_cli_signs_and_verifies ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- A generated Ed25519 key pair can sign a card; the signature verifies with the
  matching public key after a JSON round trip.
- Changing a signed field invalidates verification.
- A different trusted public key is rejected.
- An unsigned card is rejected.
- The CLI generates keys, signs, verifies, and exits non-zero for a tampered
  card or a missing trusted key.

## What this does not prove

- That the verifier obtained the public key from a trustworthy source.
- Key rotation, revocation or expiry.
- A2A registry discovery, signed registry entries or metadata exchange.
- That a signed capability claim is operationally true.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/agent_card.rs`
- `src/main.rs` (`agent-card` flags)
- `tests/agent_card_cli.rs`
- `docs/adr/0059-agent-card-signing.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
