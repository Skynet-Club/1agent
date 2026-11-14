# RFC 0002: Watchdog Automatic Repair

- Status: accepted for 0.97 (containment only)
- Date: 2026-11-12 (implemented 2026-11-13)
- Related: SA97-16, M4/M6

## Context

The watchdog currently observes, recommends `ok/warn/pause/repair`, and can
execute an explicit, checked `pause` or `resume` after a Pause/Repair
decision. It never kills, restarts or clears locks. The 0.97 status lists
"automatic watchdog repair remains manual" as a release blocker.

"Repair" is not one action. The safe options differ in authority:

1. **Containment** — pause a runaway process so it stops writing while a human
   inspects it. Uses the existing SIGSTOP path and receipt.
2. **Resume** — SIGCONT a process that was paused, usually after a heartbeat
   recovers. A paused process cannot write a fresh heartbeat, so an automatic
   resume needs a cooldown or an external health signal.
3. **Restart** — kill and relaunch the daemon. Requires lease fencing,
   in-flight side-effect handling and idempotency; otherwise it can duplicate
   work or lose locks.
4. **External repair command** — run a user-supplied program. This expands
   authority, needs an approval/policy gate and a sandbox; it is not a
   watchdog-only decision.

## Decision (proposed)

- For 0.97, implement automatic **containment** only: an opt-in `--watch` loop
  that re-evaluates the heartbeat and sends a checked SIGSTOP on a Pause/Repair
  transition, with a receipt and an intervention-log entry. It never kills,
  restarts or runs a command.
- Keep automatic **resume** explicit (`--execute resume`) until there is a
  health signal independent of the paused process.
- Move **restart** to 0.98 with lease fencing and idempotency work; do not
  claim it for 0.97.
- Do not add an external repair-command path without an approval/policy design.

## Consequences

- The blocker changes from "no automatic action" to "automatic containment,
  no automatic restart"; the limitation is explicit in release notes.
- A paused process needs a human or a separate `--execute resume`; this is a
  deliberate safety property, not an oversight.
- Any future restart/repair-command feature must first land lease fencing,
  duplicate-side-effect protection and an approval gate.

## Implementation (0.97)

`1agent watchdog --watch --execute pause` re-reads the heartbeat every
`--interval-ms` (minimum 100 ms). On the first Pause or Repair decision for a
pid it sends the same checked SIGSTOP as the one-shot path, records the
decision in `--intervention-log`, and prints the receipt. It signals a pid at
most once until the heartbeat recovers to Ok/Warn or the pid changes; a
Pause-to-Repair escalation does not signal twice. `--ticks N` bounds the loop
for tests and supervised runs; 0 runs until interrupted.

The loop refuses `--execute resume`, never sends SIGCONT, never kills or
restarts, and exits non-zero if the checked signal fails. It is still Unix
only because the validation uses `/proc/<pid>/cmdline`.

## Open questions (resolved for 0.97)

- Should `--watch` pause on both Pause and Repair, or only Repair? **Both.**
  Containment is the only automatic action; Repair does not grant more
  authority.
- Should the loop resume automatically after a fixed cooldown, or wait for an
  operator? **Wait for an operator.** A paused process cannot prove it is
  healthy, so automatic resume remains explicit.
- Which process should be the target when the heartbeat pid changes between
  ticks? **The pid in the latest heartbeat**, after the same cmdline check; the
  loop signals the new pid once and never reuses the previous target.
