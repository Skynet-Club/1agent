# Crash-Recovery Soak — Evidence (2026-11-12)

Status: **5-cycle smoke passed; 2h duration run in progress**
Scope: crash injection at checkpoint/tool-call boundaries, resume, and
duplicate-side-effect avoidance.

## Commands

```bash
SOAK_PROFILE=debug SOAK_RECOVERY_CYCLES=5 ./scripts/soak-recovery.sh
SOAK_RECOVERY_SECONDS=7200 ./scripts/soak-recovery.sh   # release profile
```

## Smoke result

```text
recovery soak: profile=debug cycles=5 seconds=0
recovery soak: 5 cycles in 22.698136ms
test crash_injection_resumes_cleanly_or_fails_closed ... ok
```

Each cycle:

- Runs a write task once and confirms the counter file has one line.
- Resumes from the first checkpoint and confirms the same conclusion, while
  the counter stays at one (the completed side effect is not replayed).
- Resumes a journal truncated at `ToolCalled` and confirms the unpaired-tool
  error, with the counter still at one.
- Resumes a journal truncated at `ToolObserved` (before the checkpoint) and
  confirms "no checkpoint is available", with the counter still at one.

## 2h run

The release binary was started at 2026-11-12 12:52 with
`SOAK_RECOVERY_SECONDS=7200` and 250 ms pacing. Its final result is appended
to this file when the run completes; the smoke result above is not a
substitute for the two-hour gate.

## What this does not prove

- A real external side effect (payment, email, remote write) or multi-agent
  contention.
- That every crash point is covered; the test injects three deterministic
  prefixes.
- That a two-hour run has passed yet; that line is pending.
