# 0.98 1h M1 — rerun after SHA/prompt fixes

Date: 2026-11-15  
Model: `deepseek-primary` / `deepseek-flash`  
Binary SHA-256: `a3f8b24e5e0787d1fd910335fcecf8074066cdc2652a659a8ca1dd2115e8d72b`  
Command: same as `../m1-1h-2026-11-15/README.md`, with
`--artifacts-dir docs/evidence/m1-1h-2026-11-15-rerun`.

## Result

```text
iterations          30
tasks_total         240
tasks_passed        231
task_completion_bps 9625   (96.25%)
completed_replay    10000  (100%)
crash_free          10000  (100%)
suite_passed        true
passed              false  (task_completion_bps < 9700)
cost_micros         10,623,037
```

The file-SHA visibility and tool-efficiency prompt recovered the
`bug-fix-last-index` and `config-retries` failures and reduced `checksum-file`
from 4 to 2, but the gate still missed by 2 tasks.

## Failure distribution (9)

| Case | Count | Observed |
|---|---|---|
| `long-task-plan` | 4 | 3 completed but answer status `conditional`; 1 budget_exhausted |
| `research-compare` | 2 | needs_evidence after an invalid answer that the repair could not fix |
| `checksum-file` | 2 | budget_exhausted (still over-verified) |
| `broken-json` | 1 | conditional (no parser run) |

## Follow-up applied after this run

The long-task-plan failures were a status-classification issue: the model
produced the requested plan but returned `conditional` because executing the
plan later would need files outside the task. The system prompt now says:
"For a plan, summary or report task, `completed` means the requested artifact
was produced from the provided material. Do not return `conditional` merely
because executing the plan later would need files or decisions outside this
task; name those as follow-ups instead."

A one-run smoke after this change is 8/8 (M1 100%). The full rerun is in
`../m1-1h-2026-11-15-rerun2/`.

## Status

This gate is not passed. Do not claim 0.98 M1 until the rerun is ≥97% or the
owner explicitly accepts the residual.
