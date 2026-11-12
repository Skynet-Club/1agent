# RFC 0002: Watchdog Automatic Repair

- Status: proposed
- Date: 2026-11-12
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

## Open questions

- Should `--watch` pause on both Pause and Repair, or only Repair?
- Should the loop resume automatically after a fixed cooldown, or wait for an
  operator?
- Which process should be the target when the heartbeat pid changes between
  ticks?
