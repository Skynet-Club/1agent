# Agent Inbox CLI and Projections — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `1agent inbox add|list|resolve|snooze`, `project_attention` and
`project_alerts`. This is a local operator surface; no TUI/desktop/chat surface
exists yet.

## Commands

```bash
cargo test inbox --lib
cargo test --test inbox_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 7 tests
...
test result: ok. 7 passed; 0 failed

running 2 tests (tests/inbox_cli.rs)
test inbox_add_list_snooze_resolve_round_trip ... ok
test inbox_rejects_invalid_kind_and_terminal_transition ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- `inbox add` creates an item and prints it as JSON.
- `inbox list` returns open or due-snoozed items, ordered review → question →
  notify.
- `inbox snooze` hides an item until its deadline.
- `inbox resolve` moves an item to a terminal status; a second transition
  fails.
- An invalid kind fails closed.
- `project_attention` maps overdue/missed commitments to `review` and due items
  to `notify`.
- `project_alerts` maps failed/error alerts to `review` and other alerts to
  `notify`.

## What this does not prove

- That items are merged from real sources into the store; `inbox sync` is not
  implemented.
- That the TUI/desktop/chat surface can act on items; only the CLI exists.
- That the inbox is tamper-evident; the store is append-only but not
  hash-chained.
- Cross-platform behavior; the test ran on Linux only.

## Files

- `src/inbox.rs` (projections, parsers)
- `src/main.rs` (`inbox` command)
- `tests/inbox_cli.rs`
- `docs/adr/0042-agent-inbox-cli-and-projections.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`

## 2026-11-12 update

`inbox list --kind KIND --source SOURCE` filters the open queue; an invalid
kind fails closed. Covered by `inbox_list_filters_by_kind_and_source`
(`cargo test --test inbox_cli`).
