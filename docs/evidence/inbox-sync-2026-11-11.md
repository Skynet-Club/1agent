# Inbox Sync — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `InboxStore::sync`, `1agent inbox sync`, and dedupe/dry-run behavior
over a real alert log.

## Commands

```bash
cargo test inbox --lib
cargo test --test inbox_sync_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 9 tests
...
test result: ok. 9 passed; 0 failed

running 1 test (tests/inbox_sync_cli.rs)
test inbox_sync_is_dry_run_by_default_then_dedupes ... ok

test result: ok. 1 passed; 0 failed
```

## What this proves

- A projected alert (`schedule_run_failed`) becomes a `review` inbox item.
- `inbox sync` without `--apply` reports `scanned/added/skipped` but writes
  nothing; `inbox list` stays empty.
- `inbox sync --apply` writes the item; `inbox list` returns it.
- A second `--apply` sync adds zero and reports the id as skipped.
- Sync does not overwrite an existing item's status.

## What this does not prove

- That attention projections from a real goal journal are merged correctly in
  a live workspace; the CLI test uses an alert log.
- That the inbox is tamper-evident; the store is append-only but not
  hash-chained.
- That a TUI/desktop/chat surface can act on items; only the CLI exists.
- Cross-platform behavior; the test ran on Linux only.

## Files

- `src/inbox.rs` (`InboxStore::sync`, `InboxSyncReport`)
- `src/main.rs` (`inbox sync`)
- `tests/inbox_sync_cli.rs`
- `docs/adr/0043-inbox-sync.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
