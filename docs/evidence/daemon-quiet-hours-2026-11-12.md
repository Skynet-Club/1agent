# Daemon Quiet Hours — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `daemon --quiet-hours` and `autonomy::local_minute`.

## Commands

```bash
cargo test --test daemon_quiet_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 2 tests (tests/daemon_quiet_cli.rs)
test daemon_rejects_invalid_quiet_hours ... ok
test daemon_skips_due_jobs_during_quiet_hours ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- A due job is not run and not marked when the daemon starts inside a quiet
  window covering the current UTC minute.
- The same job runs when the daemon is restarted without quiet hours.
- An invalid quiet window exits non-zero.

## What this does not prove

- DST or a real timezone database; a fixed UTC offset is supplied.
- Recovery of deferred work exactly at the window boundary; the next poll
  after the window runs it.
- Notifications or escalations.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/main.rs` (`daemon --quiet-hours`, `--utc-offset-minutes`)
- `src/autonomy.rs` (`local_minute`)
- `tests/daemon_quiet_cli.rs`
- `docs/adr/0062-autonomy-quiet-hours.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
