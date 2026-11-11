# Agent Card Trust Store — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `card_trust::CardTrustStore` and `1agent card-trust`.

## Commands

```bash
cargo test card_trust --lib
cargo test --test card_trust_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test card_trust::tests::duplicate_add_is_idempotent_until_revoked ... ok
test card_trust::tests::invalid_keys_and_unknown_revocations_are_rejected ... ok
test card_trust::tests::trusted_signed_cards_verify_and_survive_reopen ... ok
test card_trust::tests::unknown_revoked_and_tampered_cards_are_rejected ... ok

test result: ok. 4 passed; 0 failed

running 2 tests (tests/card_trust_cli.rs)
test card_trust_adds_verifies_and_revokes_a_key ... ok
test card_trust_rejects_invalid_keys ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- A signed card verifies against a trusted key and after store reopen.
- An untrusted embedded key is rejected.
- Editing a signed field is rejected.
- After revocation, the same card is rejected and the CLI reports `revoked`.
- Duplicate add is idempotent; adding a revoked key fails.
- Invalid key hex and revoking an unknown key fail.

## What this does not prove

- Key discovery, signed key lists or remote registry distribution.
- File-permission or OS keychain protection of the trust store.
- Concurrent writer safety or multi-process locking.
- Revocation grace periods or timestamp-based scheduling.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/card_trust.rs`
- `src/main.rs` (`card-trust`)
- `tests/card_trust_cli.rs`
- `docs/adr/0067-agent-card-trust-store.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
