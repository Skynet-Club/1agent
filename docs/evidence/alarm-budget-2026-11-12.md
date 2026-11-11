# False-Alarm Budget — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `alarm_budget::{evaluate, false_alarm_rate_bps}` and
`1agent alarm-budget`.

## Commands

```bash
cargo test alarm_budget --lib
cargo test --test alarm_budget_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 5 tests
test alarm_budget::tests::critical_alerts_bypass_budget_and_rate ... ok
test alarm_budget::tests::exhausted_budget_moves_non_critical_alerts_to_digest ... ok
test alarm_budget::tests::high_false_alarm_rate_moves_alerts_to_digest ... ok
test alarm_budget::tests::no_deliveries_has_no_rate ... ok
test alarm_budget::tests::routine_alerts_deliver_within_budget ... ok

test result: ok. 5 passed; 0 failed

running 2 tests (tests/alarm_budget_cli.rs)
test alarm_budget_cli_delivers_digests_and_bypasses_for_critical ... ok
test alarm_budget_cli_rejects_out_of_range_input ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- A routine alert within the delivery budget and false-alarm ceiling is
  delivered.
- An exhausted delivery budget moves it to the digest.
- 4000 bps with a 2000 bps ceiling moves it to the digest and reports 4000.
- A critical alert is delivered even with an exhausted budget and 100% false
  alarms.
- No deliveries means no rate; invalid CLI ranges exit non-zero.

## What this does not prove

- That the caller passes honest window counts.
- That delivered alerts are actually sent or that digests are shown.
- That user feedback marks false alarms correctly.
- Per-category budgets, learning, or trend detection.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/alarm_budget.rs`
- `src/main.rs` (`alarm-budget`)
- `tests/alarm_budget_cli.rs`
- `docs/adr/0063-false-alarm-budget.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
