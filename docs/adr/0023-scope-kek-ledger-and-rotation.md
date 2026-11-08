# ADR 0023: Scope KEK Ledger and Rotation

- Status: accepted for the A-Gate memory slice
- Date: 2026-11-08

## Context

ADR-0021 left backup-grade crypto-shredding unimplemented. The original payload
format derived a scope KEK directly from the master key, so old ciphertext could
always be decrypted while the master key existed; there was no way to retire one
scope key.

## Decision

1. New payloads use a random per-scope KEK stored in a `<key-file>-keks.json`
   ledger, wrapped by an AEAD key derived from the master key. Payload schema 2
   records `kek_version`.
2. `MemoryPayloadStore::rotate_scope_key` creates a new random version, rewraps
   every live payload in the scope, then removes older ledger versions. A backup
   that references a retired version cannot be decrypted, even with the master
   key.
3. Schema-1 payloads remain readable through the legacy master-derived key.
   Rotation migrates them to schema 2, but the legacy derivation itself cannot
   be retired while the master key exists. Legacy backups are therefore outside
   the crypto-shredding guarantee until master-key rotation.
4. The ledger is private (0600), atomically replaced and reloaded/verified on
   open. A missing or tampered version fails closed.
5. Rotation is not transactional across the whole scope: a crash may leave a mix
   of old and new payloads, but old versions are retired only after every live
   payload has been rewrapped and the payload directory is fsynced. A retry
   creates a newer version and converges.
6. In-process readers/writers take a shared rotation lock; rotation takes it
   exclusively, so a `put` cannot write an old version after retirement. Symlinked
   payloads are rejected, and the backup verifier rejects the ledger and any file
   not listed in the manifest.

## Consequences

- `hard_forget_covers_backup` remains false in backup manifests: rotation is an
  explicit operator action, not part of hard forget.
- Master-key rotation and a restore/old-backup-unrecoverable drill for legacy
  payloads are still required before any general claim that hard forget covers
  backups.
- The ledger is key material: backups must continue to exclude it, and losing it
  makes all schema-2 payloads unrecoverable.
- Single-writer assumption remains; concurrent rotation from multiple processes
  is not supported.
