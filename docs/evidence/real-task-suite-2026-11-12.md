# Real-Task Suite — Evidence (2026-11-12)

Status: **suite structurally validated; not executed against a real provider**
Scope: eight real-task definitions, per-case write permission and validate-only.

## Commands

```bash
python3 -m json.tool evals/real-tasks-v1.json >/dev/null
cargo test --offline shipped_real_task_suite_is_structurally_valid --lib
cargo test --offline validate_suite_rejects_duplicate_ids_and_empty_objectives --lib
cargo test --offline --test eval_validate_cli
```

## What this proves

- The real-task suite is valid JSON and parses as an `EvalSuite`.
- It has eight cases, `kind = model_quality`, `execution_status = ready`,
  provider requirements, a reviewer and calibration required.
- At least four cases opt into write permissions; existing suites remain
  read-only by default.
- `eval --validate-only` accepts the suite and reports eight cases, and rejects
  an empty model-quality suite without running a model.
- `eval --suite evals/real-tasks-v1.json` without `--config` fails closed with
  the model-quality provider requirement, so it cannot silently run the
  deterministic mock.
- Duplicate case ids and empty objectives fail closed.

## What this does not prove

- That a real model can complete the tasks, or any M1/M2/M3 rate.
- That the rubric terms are calibrated or that a human reviewer has scored a
  run.
- Provider behavior, cost, latency or temperature control; those are run-time
  configuration.
- That the deterministic mock is equivalent; the suite explicitly requires a
  real provider.
