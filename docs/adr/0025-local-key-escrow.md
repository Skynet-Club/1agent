# ADR 0025: Local Key Escrow

- Status: accepted for the S-Gate productization slice
- Date: 2026-11-08

## Context

ADR-0024 deliberately excludes `memory.key` and the scope KEK ledger from home
exports, so an export alone cannot decrypt memory payloads. S-Gate also requires
a backup/restore and key-escrow story. Without a way to recover key material,
losing the runtime home loses the encrypted memory even if the data export
survives.

## Decision

1. `1agent key escrow` reads `memory.key` and, when present,
   `memory.key-keks.json`, and encrypts them as one AES-256-GCM bundle.
2. A random 256-bit recovery key is generated on first use and written as
   hex to a separate file with mode 0600. The recovery key, the escrow output
   and the runtime home must be pairwise outside the home; an existing
   recovery key must be private (no group/world access), and its parent
   directory must not be group/world writable.
3. The escrow envelope contains a schema, creation time, random 96-bit nonce
   and ciphertext. The schema string is the AEAD associated data, so a rewritten
   envelope fails authentication.
4. `1agent key restore` verifies the envelope, decrypts it, checks every key
   file's size and SHA-256, rejects duplicate paths, and writes only
   `memory.key` and the ledger into an initialized current-version home. It
   refuses to overwrite existing key files and writes the ledger before the
   master key.
5. The recovery key is never printed or journaled. Tests cover a real memory
   write, key loss, escrow restore, memory re-read, tamper rejection and
   overwrite refusal.
6. This is local recovery, not organization-grade key custody. There is no
   password KDF, HSM, quorum, key rotation-on-recovery, or in-memory
   zeroization guarantee.

## Consequences

- An operator can recover encrypted memory after losing the home's key files,
  provided the escrow and recovery key were stored separately.
- A compromised recovery key compromises every master key in that escrow; the
  response is to re-escrow with a new recovery key before discarding the old
  one. Until a real key-custody design exists, R1 must not claim HSM/quorum
  escrow.
- The escrow bundle is only as current as the last escrow run. `doctor` should
  eventually warn when key files are newer than the stored escrow, but that
  policy is not implemented yet.
- Escrow reads the master key and ledger as separate files. Run it while no
  master/scope-key rotation is in progress; an interleaved rotation can capture
  an inconsistent key set.
