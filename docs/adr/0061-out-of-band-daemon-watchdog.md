# ADR 0061: Out-of-Band Daemon Watchdog

- Status: accepted
- Date: 2026-11-12

## Context

The daemon has a pid lock and catch-up scheduling, but nothing outside the
process can tell whether its loop is still alive. A hung daemon holds the lock
and looks the same as a healthy one. SA98-49 calls for an out-of-band watchdog.

## Decision

1. Add `--heartbeat PATH` to `1agent daemon`. Each loop writes
   `{pid, updated_at_ms}` atomically (temporary file + rename).
2. Add `src/watchdog.rs` with `touch`, `read` and `evaluate`. `evaluate`
   returns healthy only when a heartbeat exists and its age is at most
   `max_age_ms`.
3. Add `1agent watchdog --heartbeat PATH --max-age-ms N [--json]`. It exits
   non-zero when the heartbeat is missing, stale or invalid.
4. The watchdog is read-only with respect to the daemon. It does not restart
   the process, clear the pid lock, or kill anything.
5. Clock skew is handled conservatively: a future timestamp yields age 0
   rather than a negative age.

## Consequences

- An external scheduler (cron, systemd, another host) can detect a hung daemon
  without sharing memory or a socket.
- The heartbeat file is small and overwritten each loop; no history is kept.
- Detection is not recovery. Restart policy, lock cleanup and alert routing are
  separate operational decisions.
- A sufficiently long loop (a scheduled job exceeding the max age) can look
  stale while the daemon is working. Operators must set `max-age-ms` above the
  longest expected job.
- The heartbeat is local to one machine.

## Evidence

```bash
cargo test watchdog --lib
cargo test --test watchdog_cli
```

The tests cover fresh/stale/missing evaluation, file round-trip, invalid JSON,
and a real `daemon --once --heartbeat` that is then checked by the CLI.
