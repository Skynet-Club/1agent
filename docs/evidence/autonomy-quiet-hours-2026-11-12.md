# Autonomy Quiet Hours — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `autonomy::QuietHours`, `autonomy::decide_proactive` and
`1agent autonomy --quiet`.

## Commands

```bash
cargo test autonomy --lib
cargo test --test autonomy_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 10 tests (autonomy-related filters)
test autonomy::tests::quiet_hours_parse_and_wrap_midnight ... ok
test autonomy::tests::quiet_hours_defer_proactive_work_only ... ok
... existing autonomy tests ...

test result: ok. 10 passed; 0 failed

running 3 tests (tests/autonomy_cli.rs)
test autonomy_cli_applies_risk_and_reversibility ... ok
test autonomy_cli_defers_proactive_work_in_quiet_hours ... ok
test autonomy_cli_rejects_invalid_level_and_risk ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- `22:00-07:00` contains 23:30 and 00:00 but not 07:00 or 12:00.
- Proactive act/require_approval inside the window becomes `defer`, with the
  minutes until the window ends in the reason.
- User-initiated work and explicit approval are not deferred.
- Observe stays observe, and daytime act stays act.
- Invalid windows (`07:00-07:00`, `24:00-07:00`, `bedtime`) exit non-zero.

## What this does not prove

- That a scheduler wakes at the end of the window and runs deferred work.
- That quiet hours are persisted in the user model or onboarding.
- Time-zone/DST correctness; the caller supplies minutes since local midnight.
- That a channel honors `defer` by suppressing notifications.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/autonomy.rs`
- `src/main.rs` (`autonomy --quiet/--now/--user-initiated`)
- `tests/autonomy_cli.rs`
- `docs/adr/0062-autonomy-quiet-hours.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
