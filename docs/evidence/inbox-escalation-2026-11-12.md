# Inbox Escalation Projection and Delivery — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `1agent inbox escalate`, `escalation::EscalationState`.

## Commands

```bash
cargo test --test inbox_escalate_cli
cargo test escalation --lib
cargo test --test escalate_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test inbox_escalate_below_the_first_threshold_reports_none ... ok
test inbox_escalate_notifies_once_per_reached_rung ... ok
test inbox_escalate_rejects_invalid_ladders ... ok
test inbox_escalate_reports_only_open_items_at_their_age_level ... ok

test result: ok. 4 passed; 0 failed
```

## What this proves

- The projection applies the ladder to open items and excludes resolved ones.
- An item below the first threshold reports `level: null` with the
  `no escalation threshold reached` reason.
- Invalid thresholds/actions and non-increasing ladders exit non-zero.
- With `--webhook --state`, the first `notify` rung is delivered once; a repeat
  run against a closed port succeeds and reports `sent=false`, proving no
  second request was attempted.
- Raising the ladder to `urgent` delivers the item again with `[urgent]` in
  the body.
- `--webhook` without `--state` exits non-zero.
- The state records the delivered rung per item id.

## What this does not prove

- Delivery against a real Slack/Discord/chat endpoint; tests use a loopback
  HTTP server.
- Exactly-once delivery: a crash after the HTTP request but before the state
  save can duplicate one batch.
- `digest`-rung delivery, per-channel ladders, acknowledgement-driven stop, or
  automatic delivery from the daemon.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/escalation.rs` (`EscalationState`, `Ord` on `EscalationAction`)
- `src/main.rs` (`inbox escalate --webhook/--state/--format/--dry-run`)
- `tests/inbox_escalate_cli.rs`
- `docs/adr/0064-escalation-ladder.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
