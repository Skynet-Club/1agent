# ADR 0021: Backup Export and Hard-Forget Boundary

- Status: accepted for the A2-PR2d MVP; scope KEK rotation added in ADR-0023
- Date: 2026-11-08

## Context

A2 Memory can hard-forget a record by deleting its encrypted payload. A backup made
before the forget still contains the ciphertext. Whether that backup remains recoverable
depends on a key-escrow policy that has not been chosen. Claiming that hard forget covers
backups without such a policy would be false.

## Decision

1. Memory backup export copies the metadata journal and `*.payload.json` ciphertext only.
   It never reads or copies the master key.
2. The MVP hard-forget guarantee covers the live payload store only. Existing backups are
   explicitly outside that guarantee.
3. Every backup manifest records `key_included: false` and
   `hard_forget_covers_backup: false`; verification rejects a manifest that claims
   otherwise and rejects any `.key` file inside the backup tree.
4. Backup-grade crypto-shredding requires a later decision: scope KEK versioning, rotation
   on hard forget, key escrow, and an "old backup is unrecoverable" drill. That is not
   implemented here. ADR-0023 implements scope KEK versioning/rotation and master-key
   rotation; legacy payloads must be migrated before master rotation, and rotation is an
   explicit operator action rather than part of hard forget.

## Consequences

- Operators must treat an exported backup as potentially recoverable if the live key is
  still available; the manifest states this limitation.
- The backup format is verifiable (schema, per-file SHA-256, size, key exclusion) but is
  not a restore feature by itself.
- A future ADR must supersede this boundary before any product claim that hard forget
  covers backups.
