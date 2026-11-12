# Long-Horizon Real Benchmark — Evidence (2026-11-12)

Status: **running**
Job: `job-1794496736687-2245-2`
Collector job: `job-1794497802956-2245-4` (copies the final JSON into this
directory when the benchmark finishes)
Plan: `/home/skynet/.cache/1agent-soak/plan-bench-8h-2026-11-12.json`
Started: 2026-11-12 23:20 CST
Expected finish: 2026-11-13 07:30–08:30 CST (depending on per-run latency)

## Design

- Suite: `evals/real-tasks-v1.json` v1.1 (8 tasks).
- Runner: `1agent benchmark-run --runs 280 --config configs/pi-deepseek.json`
  with completion ≥7000 bps, crash-free ≥9950 bps, replay 10000 bps.
- Unattended: no human input, so M3 interventions/task should be 0 unless the
  process itself fails.
- The wrapper reads the DeepSeek key from `~/.pi/agent/auth.json` at runtime;
  the key is not in the plan or command line.

## Results

Append the final JSON and wall-clock duration when the job finishes:

```bash
job=job-1794496736687-2245-2
d=$HOME/.local/state/skynet-term/jobs/$job
python3 -I -c "import json;print(json.load(open('$d/state.json')))"
find "$d/runs" -name 'bench-8h.output' -exec cat {} \;
```

## What this will measure

- M1 proxy: suite completion rate over ~8 hours.
- M2: unattended wall-clock duration.
- M3: interventions/task (expected 0; the run has no human input).
- Crash-free and structural replay rates from the benchmark gate.

## What it will not prove

- A single 8-hour task; this is repeated 8-task suites.
- Human review/calibration of the suite.
- Multiple providers/models or macOS/Windows behavior.
