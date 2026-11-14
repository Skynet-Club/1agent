# Post-Repair Benchmark — Evidence (2026-11-13)

Status: **local validation pass (small samples)**

Scope: answer-repair contract (ADR-0125) and the task-level M1 / completed-run
M5 metrics (ADR-0126). The suite-level all-8 gate was disabled for these runs
(`--min-completion-bps 0 --min-replay-bps 0`) so M1 is measured per task.

## Commands

```bash
target/release/first-principles-agent benchmark-run \
  --suite evals/real-tasks-v1.json --runs 20 \
  --config configs/pi-deepseek.json --allow-network --timeout-seconds 240 \
  --min-completion-bps 0 --min-replay-bps 0 \
  --min-task-completion-bps 5000 --min-completed-replay-bps 10000 --json
```

## Results

| Run | Suites all-8 | Tasks | Task M1 | Completed cases | M5 replay | Crash-free |
|---|---:|---:|---:|---:|---:|---:|
| 5 iterations | 2/5 (40%) | 37/40 | 92.5% | 40/40 | 100% | 5/5 |
| 20 iterations | 10/20 (50%) | 144/160 | 90.0% | 157/157 | 100% | 20/20 |

The 20-run output has `passed: true` under the task-level M1 ≥50% and
completed-run M5 100% gates.

Raw output:

- `docs/evidence/repair-benchmark-2026-11-13-run5.json`
- `docs/evidence/repair-benchmark-2026-11-13-run20.json`

## What this proves

- After the tool-free repair change, both samples exceed the 0.97 M1 ≥50%
  target at task level (90% in the 20-run).
- Every completed case in the 20-run passed audit and structural replay
  (157/157), which is the M5 denominator.
- No suite crashed in either sample.

## What this does not prove

- A long-horizon result; 20 iterations are a short sample, not the 8h run.
- A second provider or human calibration.
- That the later answer-style prompt change (fewer tables/questions) has the
  same success rate; the 8h rerun must use the current binary.
- M19–M23 blind-review labels.
