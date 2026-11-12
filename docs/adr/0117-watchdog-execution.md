# ADR 0117: Explicit Watchdog Pause/Resume Execution

- Status: accepted
- Date: 2026-11-12
- Related: SA97-16

## Context

The watchdog projected an escalation ladder (ok/warn/pause/repair) and wrote an
intervention log, but every action was advisory. SA97-16 asks for pause/repair
execution without expanding authority. Automatic restart is a much larger
capability: it needs a service command, credential context and failure
semantics that the watchdog does not have.

## Decision

1. `WatchdogExecution::{Pause, Resume}` and `watchdog::execute` send SIGSTOP or
   SIGCONT only after validating the target.
2. Validation refuses pid 0, the watchdog's own pid, a missing process, an
   empty expected command, and a `/proc/<pid>/cmdline` that does not contain
   `--expected-command`. Signaling is Unix-only.
3. `1agent watchdog --execute pause|resume` requires all three escalation
   thresholds. Execution is refused for `ok` and `warn` decisions; it is
   allowed for `pause` and `repair`.
4. The command defaults to `--expected-command first-principles-agent`, so a
   recycled pid or an unrelated process is not signaled by accident.
5. The decision is still printed and the intervention log is still appended.
   In JSON mode the execution receipt goes to stderr so stdout remains one JSON
   document.
6. The watchdog never kills, restarts, clears a lock or changes authority.
   Automatic repair remains manual.

## Consequences

- Pause is reversible and explicit; resume can recover a process paused by the
  watchdog or by an operator.
- SIGCONT is not a repair for a hung process. A `repair` decision still
  requires an operator unless the process was merely stopped.
- The cmdline check and the signal are separate syscalls; a process could exec
  or exit between them (TOCTOU). The check reduces accidental targeting, not a
  hostile race.
- Windows/macOS process signaling is not implemented; the command fails
  closed there.
