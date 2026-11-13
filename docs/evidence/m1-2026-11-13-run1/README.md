# M1 20-run short regression (run 1, six-step default)

- Date: 2026-11-13
- Suite: `real-tasks-v1` v1.1 (six-step default)
- Provider: `deepseek-primary` / `deepseek-flash`
- Command: `1agent benchmark-run --suite evals/real-tasks-v1.json --runs 20
  --config configs/pi-deepseek.json --allow-network --artifacts-dir
  docs/evidence/m1-2026-11-13-run1 --json`
- Task-level M1: **146/160 = 91.25%** (gate ≥50%)
- Completed-run M5: **148/148 = 100%** (gate 100%)
- Crash-free: **100%**
- Suite-level all-8-cases: **8/20 = 40%** (historical default gate 70%; below)
- Cost: 7,554,903 micros

The command exited non-zero because of the historical suite-level 70% gate,
not the task-level M1 or M5 gates.

## Failure distribution

| Case | Failures / 20 |
|---|---|
| `bug-fix-last-index` | 7 (budget exhausted after terminal verification) |
| `broken-json` | 2 (`needs_evidence`) |
| `checksum-file` | 1 (`completed`, rubric failure) |
| `data-processing-sales` | 1 (budget exhausted) |
| `directory-summary` | 1 (`needs_evidence`) |
| `long-task-plan` | 1 (`completed`, `conditional` expected) |
| `research-compare` | 1 (`needs_evidence`) |

The dominant failure is the six-step budget: the model edits the file, then
opens a terminal to verify, and runs out of steps before the final answer.
`real-tasks-v1` v1.2 raises each case to 8 steps / 8 tool calls; run 2 tests
whether that clears the suite-level gate.

## Artifacts

- `cases-all.jsonl`: all 160 case results.
- `run-*/cases.jsonl` and `run-*/journals/`: per-run case records and raw
  journals. The journal tree is kept in the ignored directory; its SHA-256
  manifest is `journals.sha256`.
