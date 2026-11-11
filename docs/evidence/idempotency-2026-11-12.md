# Durable Idempotency Records — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `idempotency::IdempotencyStore` and the `1agent idempotency` command.

## Commands

```bash
cargo test idempotency --lib
cargo test --test idempotency_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 6 tests
test idempotency::tests::empty_and_control_tokens_are_rejected ... ok
test idempotency::tests::interrupted_append_is_ignored ... ok
test idempotency::tests::key_reuse_with_another_fingerprint_conflicts ... ok
test idempotency::tests::missing_store_is_empty ... ok
test idempotency::tests::records_survive_a_restart ... ok
test idempotency::tests::same_key_and_fingerprint_does_not_append_twice ... ok

test result: ok. 6 passed; 0 failed

running 1 test (tests/idempotency_cli.rs)
test records_replay_across_processes_and_conflicts_fail ... ok

test result: ok. 1 passed; 0 failed
```

## What this proves

- A missing store loads as empty.
- A recorded key/fingerprint/response survives dropping and reopening the store.
- Re-recording the same pair does not append a second line.
- The same key with a different fingerprint conflicts on record and replay.
- A final partial line is ignored instead of bricking the store.
- Empty or control-character tokens are rejected.
- The CLI records in one process and replays in another; a conflict or invalid
  `--response` JSON exits non-zero.

## What this does not prove

- That every external write uses the store before acting.
- Exactly-once behavior if the process crashes after the external write but
  before the record append.
- Multi-writer concurrency, file locking, compaction or retention.
- Distributed idempotency across machines.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/idempotency.rs`
- `src/main.rs` (`idempotency record|replay`)
- `tests/idempotency_cli.rs`
- `docs/adr/0060-durable-idempotency-records.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
