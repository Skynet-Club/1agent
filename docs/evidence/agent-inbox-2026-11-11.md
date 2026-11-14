# Agent Inbox Domain/Store — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `inbox::InboxItem`, `inbox::InboxStore` and the replay/status tests.
This is the domain/store slice; CLI/TUI actions and source projections are not
implemented yet.

## Commands

```bash
cargo test inbox --lib
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 5 tests
test inbox::tests::terminal_status_cannot_transition ... ok
test inbox::tests::duplicate_ids_and_invalid_items_fail_closed ... ok
test inbox::tests::snooze_hides_until_it_expires ... ok
test inbox::tests::sorting_prioritizes_review_then_question_then_notify ... ok
test inbox::tests::add_and_reload_keeps_the_latest_state ... ok

test result: ok. 5 passed; 0 failed
```

## What this proves

- Items survive a store reopen; the latest snapshot wins.
- Duplicate ids and empty required fields fail closed.
- A terminal status (`approved`, `rejected`, `answered`, `dismissed`) cannot
  transition again.
- A snoozed item is hidden from `open_sorted` until its deadline.
- Sorting is review → question → notify, then due time, then creation time.

## What this does not prove

- That the item came from a real alert, attention candidate or approval; source
  projections are the next slice.
- That a user can act on it from a CLI/TUI; no command exists yet.
- Tamper evidence; the store is append-only but not hash-chained in v0.
- Cross-platform behavior; the test ran on Linux only.

## Files

- `src/inbox.rs`
- `docs/adr/0041-agent-inbox-store.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
