# 0.98 1h M1 — partial diagnostic run (stopped)

Date: 2026-11-15  
Command: `benchmark-run --suite evals/real-tasks-v1.json --runs 30
--config configs/pi-deepseek.json --allow-network --artifacts-dir
docs/evidence/m1-1h-2026-11-15 --timeout-seconds 240 --max-cost-micros
20000000 --min-task-completion-bps 9700 --min-completed-replay-bps 10000`

## Why it was stopped

After 13 runs / 105 cases, 96 passed (M1 91.43%). The gate allows at most 7
failures out of 240; 9 failures had already occurred, so 97% was
mathematically unreachable. The run was stopped to avoid spending the
remaining provider budget on an invalid gate result. This is a diagnostic
record, not a release claim.

## Failure distribution

| Case | Count | Observed |
|---|---|---|
| `checksum-file` | 3 | 2 completed but rubric evidence source was a tool id, 1 needs_evidence |
| `long-task-plan` | 2 | conditional |
| `bug-fix-last-index` | 1 | budget_exhausted after 8 provider calls |
| `config-retries` | 1 | budget_exhausted after 8 provider calls |
| `data-processing-sales` | 1 | needs_evidence after 8 provider calls |
| `broken-json` | 1 | needs_evidence |

The 0.97 M1 run had the same `checksum-file`/`long-task-plan`/`broken-json`
failure classes but fewer budget-exhausted cases.

## Root cause found

The eval harness created each case workspace with `tempfile::tempdir()`, so the
objective embedded a random path such as `/tmp/.tmproQPcr/agent.conf`. The
model mistyped the random segment (`tmporQPcr`, `tmpoQPcr`, `tmprQPcr`) and
burned its 8 provider attempts retrying paths. This is a harness noise source,
not a runtime permission failure: the first `read_file` succeeded, the edits
went to invented paths.

## Follow-up fix

When an artifacts directory is available, the eval harness now uses a clean,
deterministic **short absolute** workspace at
`/tmp/1agent-eval/<run-key>/<case-id>` instead of a random temp directory.
The first attempt used `<artifacts>/workspaces/<case-id>`; because the
artifacts path was relative and long, the model still looped, so the final fix
keeps the path short and absolute. `eval::case_workspace` removes stale
contents first; the unit test `artifact_workspace_is_deterministic_and_clean`
covers it. The runtime and tool permissions are unchanged.

## Validation of the fix

A one-run smoke (`--runs 1`, 8 cases) after the short absolute path change:

- `tasks_total=8`, `tasks_passed=8`, `task_completion_bps=10000`,
  `completed_replay_bps=10000`, `passed=true`, cost 356,565 micros.
- `config-retries` used the exact path in three calls:
  `read_file` → `edit_file` → `read_file`, and completed with
  `retries` changed from 1 to 3.
- The previously failing `bug-fix-last-index`, `data-processing-sales`,
  `long-task-plan`, `broken-json` and `checksum-file` all passed.

The full rerun still shows occasional non-path failures (for example
`checksum-file` over-verifying and exhausting its 8-step budget, or
`config-retries` omitting the rubric's flip condition). Those are model
behavior/step-budget findings, not the random-workspace bug; the full rerun
result is in `docs/evidence/m1-1h-2026-11-15/`.

## Next

Rebuild the release binary and rerun the full 30-run M1 into
`docs/evidence/m1-1h-2026-11-15/`. The rerun result is the gate evidence.
