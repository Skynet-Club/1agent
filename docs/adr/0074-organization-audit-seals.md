# ADR 0074: Organization Audit Seals

- Status: accepted
- Date: 2026-11-12

## Context

The organization audit index is a SHA-256 hash chain, but an attacker who can
rewrite the whole file can recompute the chain. A verifier needs a head
commitment that is signed and stored outside the log.

## Decision

1. Add `OrgAuditSeal { algorithm, entries, head_hash, sealed_at_ms,
   public_key, signature }`.
2. `seal` signs the canonical body (`algorithm`, entry count, head hash,
   sealed time) with an Ed25519 PKCS#8 key. The empty store seals the genesis
   hash.
3. `verify_seal(seal, trusted_public_key)` fails closed when:
   - the algorithm is not `ed25519`;
   - the sealed entry count differs from the store;
   - the sealed head hash differs from the store;
   - the embedded public key differs from the trusted key;
   - the signature is malformed or does not verify.
4. Add `org-audit generate-key` (writes a 0600 private key file and an optional
   public key file), `org-audit seal --key FILE --out FILE`, and
   `org-audit verify --seal FILE --public-key HEX`. JSON verification adds
   `seal_verified`.
5. A seal proves the log matched the sealed state at seal time. It does not
   authenticate actors, prevent deletion of both log and seal, or replace
   remote anchoring.

## Consequences

- Appending after sealing is detected until a new seal is made.
- Replacing the whole log with a recomputed chain is detectable only if the
  verifier holds the seal/public key independently.
- The private key is read from a file with the same 0600 check as other key
  files; it is not passed in argv.
- Seals are point-in-time; automatic periodic sealing and retention are not
  implemented.
- Seal files are not themselves stored in an append-only log.

## Evidence

```bash
cargo test org_audit --lib
cargo test --test org_audit_seal_cli
```

The tests cover sealing, entry-count/head binding, wrong key, tampering and
CLI key generation/seal/verify.
