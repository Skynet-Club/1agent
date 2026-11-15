# ADR-0136: Durable scheduled-run ledger and supervision

- Status: accepted for 0.98
- Date: 2026-11-15
- Supersedes: none
- Related: ADR-0061 (out-of-band watchdog), ADR-0118 (kill switch),
  `src/schedule_ledger.rs`, `src/service_unit.rs`, `docs/0.98-progress.md`

## Context

The 0.97 daemon stored `last_run_ms`/`next_run_ms` in a schedule JSON file.
A crash between “became due” and “marked run” could repeat a slot after
restart; there was no durable record of an in-flight run, no systemd unit
generator and no supervised readiness/watchdog notification. The 0.98 gate
requires at-most-once scheduled runs across hard crashes and an auditable
restart story.

## Decisions

1. **Run identity is `(schedule_id, scheduled_at_ms)`.** A trigger-file run
   uses the trigger mtime as the slot.
2. **Claim before spawn.** `RunLedger::claim` appends and `sync_data`s the
   record before the child process starts. A duplicate claim returns
   `AlreadyClaimed` and the daemon records `duplicate-skipped`.
3. **Statuses are append-only.** `claimed`, `running`, `completed`, `failed`,
   `interrupted`; the latest record per slot wins on replay. A torn tail line
   is ignored.
4. **Restart reconciles to `interrupted`.** Claimed/running records found on
   startup are marked interrupted and are **not** automatically retried. The
   default policy is at-most-once, not at-least-once.
5. **Jitter is deterministic.** 0–5% of the interval is derived from the job
   id, so restarts do not change the schedule.
6. **Missed-run policy is explicit.** The daemon defaults to catching up the
   latest overdue slot; `--missed-run-skip-after-seconds` records
   `missed-skipped` instead. Bounded/all catch-up variants are not implemented
   in 0.98.
7. **Supervision is a user unit, not an auto-enabled service.**
   `daemon --print-unit/--write-unit` renders a hardened unit; `--write-unit`
   never runs `enable` or `enable-linger`. `sd_notify` sends `READY=1` and
   `WATCHDOG=1` through the filesystem socket; abstract sockets are not
   supported.
8. **No automatic resume/kill.** A hung run is reported; the watchdog may
   pause, never silently restart or kill a user process.

## Consequences

- A crash after claim loses that slot's run by design; the operator sees an
  `interrupted` record instead of a duplicate side effect.
- The ledger grows append-only; operators may rotate it after all records for
  a slot are terminal.
- A systemd unit does not prove the scheduler is installed; the owner must
  enable it explicitly.

## Tests

- `src/schedule_ledger.rs`: claim/reconcile/torn-tail and 100-slot matrix.
- `tests/ledger_crash.rs`: 100 real child processes claim then `_exit(137)`;
  restart reconciles and cannot re-claim.
- `src/service_unit.rs`: hardened unit rendering, no linger, path escaping.
- `tests/watchdog_cli.rs`: stale heartbeat, pause-once, no auto-resume.
