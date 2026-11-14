# Watchdog Escalation Ladder — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `WatchdogPolicy`, `decide`, intervention logging and the `watchdog` CLI.

## Commands

```bash
cargo test watchdog --lib
cargo test --test watchdog_cli
```

## What this proves

- Thresholds must be positive and strictly increasing; `0, 2000, 4000`,
  `2000, 2000, 4000` and `2000, 3000, 3000` are rejected.
- A fresh heartbeat maps to `ok`; ages just over each threshold map to
  `warn`, `pause` and `repair`; a missing heartbeat maps to `repair`.
- The intervention log records `warn`, then skips an identical `warn`, records
  `pause`, records one recovery to `ok`, and skips a repeated `ok`.
- The CLI prints a `WatchdogDecision` only when all three thresholds are set,
  keeps the old status shape otherwise, exits non-zero for `repair`, and exits
  zero after recovery. Re-running the same action does not duplicate the log.
- An invalid threshold order is rejected by the CLI with a non-zero exit.

## What this does not prove

- That the watchdog actually pauses or repairs a daemon; these are
  recommendations and no signal or restart is sent.
- That the heartbeat corresponds to a live process or that the PID is the
  daemon; only its age is evaluated.
- Log rotation, retention or tamper resistance; the file is append-only JSONL
  written by the calling process.
- Behavior under a concurrent watchdog writer; there is no cross-process lock.
