# M1 20-run short regression (run 2, eight-step budget)

- Date: 2026-11-13/14
- Suite: `real-tasks-v1` v1.2 (8 steps / 8 tool calls per case)
- Provider: `deepseek-primary` / `deepseek-flash`
- Command:

```bash
PI_DEEPSEEK_API_KEY=... 1agent benchmark-run \
  --suite evals/real-tasks-v1.json --runs 20 \
  --config configs/pi-deepseek.json --allow-network \
  --artifacts-dir docs/evidence/m1-2026-11-13 \
  --min-completion-bps 5000 --min-task-completion-bps 5000 \
  --min-completed-replay-bps 10000 --json
```

- Suite-level all-8-cases: **11/20 = 55%** (0.97 target ≥50%)
- Task-level M1: **149/160 = 93.12%** (gate ≥50%)
- Completed-run M5: **154/154 = 100%** (gate 100%)
- Crash-free: **100%**
- Cost: 8,358,567 micros
- CLI result: **passed = true**

## Failure distribution

| Case | Failures / 20 |
|---|---|
| `bug-fix-last-index` | 5 (model verification/answer shape) |
| `long-task-plan` | 3 (status/answer shape) |
| `broken-json` | 2 (`needs_evidence`) |
| `data-processing-sales` | 1 (budget/answer shape) |

The eight-step budget raised the suite-level result from 8/20 (run 1) to
11/20 while keeping task-level M1 at 93.12% and completed-run replay at 100%.

## Artifacts

- `cases-all.jsonl`: all 160 case results.
- `run-*/cases.jsonl` and `run-*/journals/`: per-run case records and raw
  journals. The journal tree is kept in the ignored directory; its SHA-256
  manifest is `journals.sha256`.

## Known limits

- This is a 20-run short regression, not the 8h long-horizon gate.
- One provider/model only.
- The 0.97 suite-level target (≥50%) passes; the historical 70% default does
  not. The task-level M1 gate is the primary 0.97 metric.
