# ADR 0124: Opt-In Watchdog Containment Loop

- Status: accepted
- Date: 2026-11-13
- Related: SA97-16, RFC 0002, ADR-0117, M4/M6

## Context

The watchdog can project `ok`/`warn`/`pause`/`repair` and execute one checked
SIGSTOP/SIGCONT on request, but a one-shot command leaves containment to an
operator who must rerun it. RFC 0002 separates containment from repair: pausing
a process is reversible and bounded, while restart or an external repair
command needs lease fencing, duplicate-side-effect protection and an approval
gate.

## Decision

1. Add `1agent watchdog --watch --execute pause`. `--watch` requires all three
   escalation thresholds and an explicit `--execute pause`; `--execute resume`
   is rejected in watch mode.
2. Re-read the heartbeat every `--interval-ms` (minimum 100 ms). `--ticks N`
   bounds the loop for tests and supervised runs; 0 runs until interrupted.
3. On a Pause or Repair decision with a pid, send the same checked SIGSTOP as
   the one-shot path: refuse self, pid 0, a missing process and a cmdline that
   does not contain `--expected-command`.
4. Signal a pid at most once until the heartbeat recovers to Ok/Warn or the pid
   changes. A Pause-to-Repair escalation does not signal twice.
5. Append the existing intervention record on action changes, print a receipt
   for each signal, and emit one line of JSON per tick with `--json`.
6. Never send SIGCONT, kill, restart, clear a lock or run an external repair
   command. Exit non-zero if the checked signal fails.

## Consequences

- A stale daemon can be contained without an operator watching the clock, but a
  paused process still needs an explicit `--execute resume` or human action.
- The heartbeat cannot prove health while the process is stopped, so automatic
  resume remains unsupported; it needs an independent health signal.
- The loop is Unix-only because validation reads `/proc/<pid>/cmdline`. The
  cmdline check and signal are separate syscalls (TOCTOU), and the intervention
  log has no rotation.
- Automatic restart is deferred to 0.98 with lease fencing, idempotency and an
  approval gate. 0.97 release notes must say “automatic containment, no
  automatic restart”.
