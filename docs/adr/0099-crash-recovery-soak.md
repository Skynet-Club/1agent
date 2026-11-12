# ADR 0099: Crash-Recovery Soak

- Status: accepted
- Date: 2026-11-12

## Context

The normal soak covered store and journal growth but never injected a crash
at a checkpoint boundary or mid-tool-call, and never proved that a completed
side effect is not replayed on resume. SA97-24 requires a 2h soak with crash
injection, recovery and no duplicate side effects.

## Decision

1. Add an ignored integration test that runs a deterministic write task, then
   copies journal prefixes into three interrupted journals:
   - through the first checkpoint: resume must complete with the same answer
     and must not execute the side effect again;
   - through `ToolCalled` but before its observation: resume must fail closed
     with the unpaired-tool-call error and must not execute the side effect;
   - through `ToolObserved` but before the checkpoint: resume must fail closed
     with "no checkpoint is available".
2. Count side effects by appending one line to a per-cycle counter file.
   Assert the count stays at one in every truncation case.
3. Support two modes: `SOAK_RECOVERY_CYCLES` (default 5) and
   `SOAK_RECOVERY_SECONDS` (duration mode, paced at 250 ms per cycle). The
   duration mode exists so a release gate can run the literal two hours
   without creating millions of temporary journals.
4. Add `scripts/soak-recovery.sh`; it runs the release profile by default and
   can select debug for a quick check.

## Consequences

- The soak proves the checkpoint-resume contract and the fail-closed behavior
  around unpaired tool calls, not just that the process stays alive.
- A completed side effect is not replayed when resuming from its checkpoint.
  An interrupted side effect is never silently retried; the operator must
  inspect the journal.
- The 2h duration run is a separate evidence line. A 5-cycle run is a smoke
  check, not a substitute for the two-hour gate.
- The test uses a deterministic local provider and a single write tool; it does
  not represent a real external side effect or multi-agent contention.
