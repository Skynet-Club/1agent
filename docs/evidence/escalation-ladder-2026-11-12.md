# Escalation Ladder — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `escalation::{EscalationLadder, plan}` and `1agent escalate`.

## Commands

```bash
cargo test escalation --lib
cargo test --test escalate_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests (escalation module)
test escalation::tests::a_grace_period_keeps_the_item_quiet ... ok
test escalation::tests::invalid_ladders_are_rejected ... ok
test escalation::tests::picks_the_highest_reached_step ... ok
test escalation::tests::resolved_items_do_not_escalate ... ok

test result: ok. 4 passed; 0 failed

running 2 tests (tests/escalate_cli.rs)
test escalate_cli_picks_the_reached_rung ... ok
test escalate_cli_rejects_bad_steps ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- With thresholds 0/900000/3600000, an item aged 900000 ms selects `notify`.
- A resolved item selects no level even after the top threshold.
- Before the first threshold, no level is selected.
- Empty, equal-threshold and unknown-action ladders are rejected by the CLI.

## What this does not prove

- That any escalation is actually delivered.
- That acknowledgement/resolution is persisted or passed correctly by a caller.
- Per-channel or per-kind ladders.
- That the chosen thresholds fit real users.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/escalation.rs`
- `src/main.rs` (`escalate`)
- `tests/escalate_cli.rs`
- `docs/adr/0064-escalation-ladder.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
