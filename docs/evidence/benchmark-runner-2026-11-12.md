# Benchmark Runner — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `1agent benchmark-run` and the benchmark gate mapping.

## Commands

```bash
cargo run --offline --quiet -- benchmark-run --suite evals/core.json --runs 2 --json
cargo test --test benchmark_run_cli
```

## Result

```json
{
  "iterations": 2,
  "completed": 2,
  "crashed": 0,
  "replay_verified": 2,
  "completion_bps": 10000,
  "crash_free_bps": 10000,
  "replay_bps": 10000,
  "passed": true
}
```

A one-case suite that expects `completed` but receives `needs_evidence`
produces `completed=0`, `completion_bps=0`, `passed=false`, and a non-zero
exit with `benchmark gate failed`.

## What this proves

- A suite can be repeated in-process and its results feed the existing gate.
- A failing suite fails the gate and exits non-zero.
- Crash-free and replay samples are produced from suite reports.
- The run count and JSON shape are stable.

## What this does not prove

- Real 8h/M1 performance; the suite is a contract fixture with a
  deterministic provider.
- Process-crash accounting; a crash aborts the runner.
- Model determinism; replay is structural report/audit validation.
- That runs are independent when a configured provider keeps state.
