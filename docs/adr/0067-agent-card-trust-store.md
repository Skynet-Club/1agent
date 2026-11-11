# ADR 0067: Agent Card Trust Store

- Status: accepted
- Date: 2026-11-12

## Context

Agent Card signing (ADR-0059) verifies a card against one public key passed on
the command line. That does not model key rotation or revocation: a peer needs
a set of trusted keys and a way to retire one.

## Decision

1. Add `TrustedKey { public_key, note, added_at_ms, revoked_at_ms,
   revoked_reason }` and `CardTrustStore { keys }` in `src/card_trust.rs`.
2. The store is a JSON file. `open` treats a missing file as an empty store.
   Writes use a temporary file, `sync_all`, then rename.
3. `add` normalizes the key to 32-byte hex. Re-adding an active key is
   idempotent and keeps the existing note. Re-adding a revoked key fails;
   revocation cannot be undone by add.
4. `revoke` records `revoked_at_ms` and an optional reason. Revocation is
   immediate for verification regardless of the timestamp.
5. `verify` requires a signed card, an embedded key that is trusted and not
   revoked, then delegates the signature check to `agent_card::verify_card`.
6. `1agent card-trust add|revoke|list|verify` exposes the store. A rejected
   card exits non-zero.
7. This is a local trust anchor. Key discovery, signed key lists, expiry and
   remote registry updates are separate.

## Consequences

- Rotation is add-new-then-revoke-old; a card signed by the old key stops
  verifying immediately after revocation.
- The store is as sensitive as a keyring: anyone who can write it can add a
  key. File permissions and OS keychain integration are not implemented.
- Concurrent writers can race on the read-modify-write cycle; no lock is taken.
- Revocation has no grace period and no timestamp comparison; that is
  deliberate until a real distribution channel exists.
- Revoked keys remain in the list for audit.

## Evidence

```bash
cargo test card_trust --lib
cargo test --test card_trust_cli
```

The tests cover add/verify/reopen, unknown keys, tampered cards, revocation,
invalid keys, unknown revocation, idempotent add and revoked-key re-add.
