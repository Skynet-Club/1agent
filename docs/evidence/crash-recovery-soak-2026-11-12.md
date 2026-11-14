# Crash-Recovery Soak — Evidence (2026-11-12)

Status: **5-cycle smoke passed; 2h rerun passed with raw output**
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
`SOAK_RECOVERY_SECONDS=7200` and 250 ms pacing. A second start was observed at
15:49; the evidence for the exact process lifetime is incomplete.

The background wait task reported **completed, exit 0** at 17:51. The raw
`/tmp/soak-recovery-2h.log` and the task output file were removed before they
could be appended here, so this file records the completion signal but does not
contain the raw soak output. The smoke result above is not a substitute for the
two-hour gate.

## Rerun

A fresh 2h soak was submitted at 2026-11-12 20:19 as job
`job-1794485959622-2245-1` (release profile, `SOAK_RECOVERY_SECONDS=7200`,
timeout 7800s). It finished at 22:21 CST with **exit 0**, wall clock 7315.4s:

```text
recovery soak: 28552 cycles in 7200.197629806s
test crash_injection_resumes_cleanly_or_fails_closed ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7200.20s
```

Raw output (19,139 bytes, sha256
`282a51e51bd3f4610f0c1812718e669caed7fcd6c090a4a61a389d8c0bec2110`):

```text
docs/evidence/crash-recovery-soak-2026-11-12/soak-2h.output
```

The soak binary was built at 20:19 from the tree based on `7d12f0d`, before
the later model-request-delta, TUI-unslop and Clarify commits. Those changes do
not touch checkpoint, resume or tool-pairing code, so M4's recovery path is
unchanged; release evidence should still name the base commit.

## What this does not prove

- A real external side effect (payment, email, remote write) or multi-agent
  contention.
- That every crash point is covered; the test injects three deterministic
  prefixes.
- That the first 2h run's raw output existed at review time; only the
  completion signal survives.
- Real-crash telemetry: the rerun is a deterministic injection loop, not a
  sample of organic crashes.
