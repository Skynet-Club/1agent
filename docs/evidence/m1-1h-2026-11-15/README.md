# 0.98 1h M1 — full 30-run result

Date: 2026-11-15  
Model: `deepseek-primary` / `deepseek-flash`  
Binary SHA-256: `7e63361e615d5418b440208f24a30e67fa3062956de84e8457d060edbdbc04c0`  
Command:

```bash
benchmark-run --suite evals/real-tasks-v1.json --runs 30 \
  --config configs/pi-deepseek.json --allow-network \
  --artifacts-dir docs/evidence/m1-1h-2026-11-15 \
  --timeout-seconds 240 --max-cost-micros 20000000 \
  --min-task-completion-bps 9700 --min-completed-replay-bps 10000
```

## Result

```text
iterations          30
tasks_total         240
tasks_passed        229
task_completion_bps 9541   (95.41%)
completed_replay    10000  (100%)
passed              false
cost_micros         12,112,112
```

**M1 gate: FAILED.** The 0.98 target is ≥97%; this run is 95.41%.

## Failure distribution (11)

| Case | Count | Observed |
|---|---|---|
| `checksum-file` | 4 | `budget_exhausted`; over-verified with terminal create/exec/destroy |
| `bug-fix-last-index` | 2 | 1 conditional, 1 budget_exhausted |
| `long-task-plan` | 2 | 1 conditional, 1 budget_exhausted |
| `research-compare` | 1 | needs_evidence |
| `directory-summary` | 1 | needs_evidence |
| `config-retries` | 1 | completed but missing the rubric's flip condition |

The 0.97 run had 6 failures (233/240) with the same suite; the extra failures
here are mostly step-budget exhaustion on small file tasks, not path loops.
The earlier random-workspace path issue is fixed in
`docs/evidence/m1-1h-2026-11-15-partial/`.

## Follow-up applied after this run

- `write_file` / `edit_file` observations now include the file SHA-256 in the
  model-visible summary (previously only in the content/metadata), so a task
  that asks for a file hash does not need to open a terminal to recompute it.
- The system prompt now tells the model to cite the write/edit SHA-256, not
  re-read or run a compiler just to confirm, and never to call `request_tools`
  with an empty list.
- These target the four `checksum-file` budget failures and the
  `bug-fix-last-index` over-verification. A one-run smoke and a full rerun are
  required before this result can be replaced as the gate evidence.

## Status

This file records the failed gate honestly. Do not claim 0.98 M1 as passed
until a rerun is ≥97% or the owner explicitly accepts the residual.
