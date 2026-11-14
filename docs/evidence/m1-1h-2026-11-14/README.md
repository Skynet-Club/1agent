# 1-hour DeepSeek task-quality test

- Date: 2026-11-14
- Suite: `real-tasks-v1` v1.2 (8 cases per run, 8 steps / 8 tool calls per case)
- Provider: DeepSeek `deepseek-flash` via `configs/pi-deepseek.json`
- Runs: 30 × 8 = 240 tasks
- Start: 16:19:43 CST; end: 17:03:59 CST; duration **44m16s** (inside the 1h cap)
- Command: `benchmark-run --suite evals/real-tasks-v1.json --runs 30
  --config configs/pi-deepseek.json --allow-network
  --artifacts-dir /tmp/1agent-1h-task-artifacts
  --max-cost-micros 20000000 --min-completion-bps 5000
  --min-task-completion-bps 5000 --min-completed-replay-bps 10000 --json`

## Result

- Task-level M1: **233/240 = 97.08%** (gate ≥50%)
- Suite-level all-8-cases: **23/30 = 76.66%** (≥50%; above the historical 70% default)
- Completed-run M5: **239/239 = 100%**
- Crash-free: **30/30 = 100%**
- Cost: **11,633,661 micros ≈ ¥11.63** (cap ¥20)
- Historical suite audit+replay gate: **29/30 = 96.66%**, below the default 100%.
  This is why the CLI reports `passed=false` / exit 1.

## The one integrity miss

`run-0020` / `bug-fix-last-index` ended with `provider_failed` and
`run_stopped provider_error`. The harness did not write a case record for that
early error path, so `cases-all.jsonl` has 239 of 240 records. The task-level
M1 conservatively counts the missing case as failed (233/240). No process
crashed; the other 29 suites had every recorded case audit/replay clean.

## Recorded case misses

- `long-task-plan`: 3 × conditional where completed was expected
- `checksum-file`: 2 × conditional where completed was expected
- `broken-json`: 1 × conditional where completed was expected

These are answer-shape/model-quality misses, not crashes. The completed-run
replay denominator is the 239 recorded completed cases.

## Artifacts

- `cases-all.jsonl`: 239 recorded case results.
- `journals.sha256`: digest of the 239+ raw journals under
  `/tmp/1agent-1h-task-artifacts`.
- Report: `docs/evidence/m1-1h-2026-11-14.json`.

## Conclusion

The 0.97 task-quality target passes (M1 97.08%, suite 76.66%, M5 100%). The
only non-green signal is the historical suite audit+replay 100% gate, caused by
one provider error whose case record was lost in the early-error path. That is
a harness artifact gap to fix, not a runtime crash or a task-quality failure.
