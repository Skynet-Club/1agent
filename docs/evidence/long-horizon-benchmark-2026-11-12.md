# Long-Horizon Real Benchmark — Evidence (2026-11-12)

Status: **FAILED — release gate not met**
Job: `job-1794496736687-2245-2`
Collector job: `job-1794497802956-2245-4`
Plan: `/home/skynet/.cache/1agent-soak/plan-bench-8h-2026-11-12.json`
Started: 2026-11-12 23:20 CST
Finished: 2026-11-13 06:32 CST
Wall clock: 26,029.1 s (7 h 13 m 49 s)

## Design

- Suite: `evals/real-tasks-v1.json` v1.1 (8 tasks).
- Runner: `1agent benchmark-run --runs 280 --config configs/pi-deepseek.json`
  with completion ≥7000 bps, crash-free ≥9950 bps, replay 10000 bps.
- Unattended: no human input, so M3 interventions/task should be 0 unless the
  process itself fails.
- The wrapper reads the DeepSeek key from `~/.pi/agent/auth.json` at runtime;
  the key is not in the plan or command line.

## Results

```json
{
  "iterations": 280,
  "completed": 136,
  "crashed": 0,
  "replay_verified": 238,
  "completion_bps": 4857,
  "crash_free_bps": 10000,
  "replay_bps": 8500,
  "passed": false
}
```

Configured gate: completion ≥7000 bps, crash-free ≥9950 bps, replay 10000 bps.

- Completion **4857 bps (48.57%)** — failed the configured 7000 bps (1.0
  target). The 0.97 target for M1 is ≥50%, so this is also 1.43 percentage
  points below the 0.97 target.
- Replay **8500 bps (85.00%)** — below the configured 10000 bps, but this
  metric counts every suite attempt, including suites with invalid model
  output. M5 is defined over **completed** runs; the 42 non-replay-verified
  iterations are the invalid-output cases, not evidence that completed
  journals fail replay. M5 still needs a completed-run calculation.
- Crash-free **10000 bps (100%)** — passed.
- Crashed 0 / 280.

Raw JSON: `docs/evidence/long-horizon-benchmark-2026-11-12.json`
(sha256 `ea06c3df541b7b32e6579e7dc5d1c8a586e0deb82be5c32c54452b5da59a4449`).
The collector copied the benchmark stdout including the trailing
`Error: benchmark gate failed`; the JSON block was extracted and the artifact
is otherwise unchanged.

## Diagnosis

A 3-run diagnostic sample (24 task executions) passed 21/24 tasks (87.5%) but
only 1/3 all-8 suites. Failures were model-contract violations, not crashes:

- automatic answer repair returned tool calls instead of final JSON;
- a completed answer relied on a high-impact assumption and the automatic
  repair did not produce a valid answer;
- a checksum task returned `Conditional` where the rubric expects `Completed`;
- an earlier diagnostic run cited a prose tool-result description instead of
  the exact path `checksum.txt`, so the rubric source check failed.

The benchmark's all-task completion metric compounds per-task failures; at
about 87.5% per-task reliability, an 8-task suite passes only about half the
time. The 42 non-replay-verified iterations are the same invalid-output cases:
a failed case has no auditable terminal answer, so `replay_verified` is false.
This is a reliability/product-quality gap, not evidence of journal corruption.

## What this means

- 0.97.0 must not be released with these numbers: M1 is below its 0.97 ≥50%
  target and far below the configured 70% gate.
- M2 autonomy duration is 7 h 13 m 49 s unattended, but the run failed its
  quality gate; M3 interventions were 0.
- M5 is not established by the 85% benchmark replay number; calculate replay
  only over completed runs before calling the hard gate.
- The next iteration should target the repair contract (disable tools during
  answer repair, handle high-impact assumptions, exact evidence paths), then
  rerun a shorter benchmark to measure improvement before another 8h run.

## What this will measure

- M1 proxy: suite completion rate over ~8 hours.
- M2: unattended wall-clock duration.
- M3: interventions/task (expected 0; the run has no human input).
- Crash-free and structural replay rates from the benchmark gate.

## What it will not prove

- A single 8-hour task; this is repeated 8-task suites.
- Human review/calibration of the suite.
- Multiple providers/models or macOS/Windows behavior.
