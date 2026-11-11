# ADR 0060: Durable Idempotency Records

- Status: accepted
- Date: 2026-11-12

## Context

The Surface API has an in-memory `IdempotencyRegistry`, and several workers
carry idempotency keys, but a process restart loses the mapping from key to
response. A retry after a crash can repeat an external write that already
succeeded. SA99-58 requires cross-restart dedupe.

## Decision

1. Add `src/idempotency.rs` with `IdempotencyStore`, keyed by an idempotency
   key and a request fingerprint.
2. Each record is one JSON line:
   `{ key, fingerprint, response }`; `response` is any JSON value.
3. `record` appends and calls `sync_data` before returning. Re-recording the
   same key and fingerprint returns the existing record without another append.
4. `open` reloads the log. The same key with a different fingerprint is a
   conflict at load or replay. A final line without a newline is treated as an
   interrupted append and ignored.
5. Empty or control-character keys/fingerprints are rejected; tokens are
   limited to 512 characters.
6. Add `1agent idempotency record|replay --store PATH --key ... --fingerprint
   ...`. A conflict exits non-zero; a replay miss is a normal `hit: false`.
7. This is a storage primitive. It does not itself wrap tool calls or prove
   that every external write uses a key. That wiring is a separate slice.

## Consequences

- A retry after a restart can return the recorded response instead of repeating
  the write, when the caller checks the store first.
- The store is append-only and survives partial final writes.
- Duplicate keys with different fingerprints fail closed rather than guessing.
- The log grows forever. Compaction/retention is not implemented.
- The store is local to one machine; multi-writer coordination and distributed
  exactly-once semantics are out of scope.
- A crash between the external write and the record append still requires the
  external system's own idempotency support.

## Evidence

```bash
cargo test idempotency --lib
cargo test --test idempotency_cli
```

The tests cover missing stores, restart replay, duplicate suppression, key
reuse with a different fingerprint, interrupted appends, token validation, and
a CLI record/replay/conflict flow across processes.
