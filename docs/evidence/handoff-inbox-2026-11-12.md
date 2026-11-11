# Handoff Inbox Projection — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `inbox::project_handoffs` and
`1agent inbox sync --handoff-store`.

## Commands

```bash
cargo test inbox --lib
cargo test --test handoff_inbox_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 12 tests (inbox module)
test inbox::tests::handoff_projection_filters_recipient_and_terminal_items ... ok
... existing inbox tests ...

test result: ok. 12 passed; 0 failed

running 2 tests (tests/handoff_inbox_cli.rs)
test handoffs_project_into_the_recipient_inbox_once ... ok
test recipient_requires_a_handoff_store ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- An open handoff to bob becomes one `review` item `handoff:h1`.
- Accepted handoffs, handoffs to other people and overlong summaries are
  filtered or truncated to 500 characters.
- Sync is dry-run first, adds one item on apply and skips it on the second
  apply.
- Syncing as a different recipient scans zero items.
- `--recipient` without `--handoff-store` exits non-zero.

## What this does not prove

- That the recipient identity is authenticated.
- That the recipient is notified outside the inbox.
- That accepting from the inbox is wired; acceptance remains `handoff accept`.
- That dismiss/resolve transitions map back to the handoff store.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/inbox.rs`
- `src/main.rs` (`inbox sync --handoff-store/--recipient`)
- `tests/handoff_inbox_cli.rs`
- `docs/adr/0069-handoff-inbox-projection.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
