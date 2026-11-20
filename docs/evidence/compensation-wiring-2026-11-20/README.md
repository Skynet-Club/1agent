# MA-009 compensation wiring (2026-11-20)

Status: **landed for Master; Subagent side effects remain journal-only**

## Scope

Master now records applied artifact side effects and builds a repair runbook
when a task fails. It reuses the shared `SideEffect` / `RepairRunbook` types and
the hash-chained `MasterStore` journal; it does not duplicate a second
`GovernedTaskGraph` ticket/mandate/approval store on top of Master's existing
plan approval, lease and independent-verification gates.

## Landed

- A verified Work Item records one applied, reversible `SideEffect` per expected
  artifact: action `write_artifact`, target `<workspace>/<artifact>`,
  idempotency key `<task_id>:<artifact>`, compensation
  `remove <target> only if this task created it; restore any previous file
  otherwise`.
- Effect recording is idempotent; re-verification does not duplicate an effect.
- A rejected verification, a failed/interrupted/cancelled receipt, or a blocked
  observation computes `repair::build_runbook` over the trace's applied effects
  and stores it on `TaskRuntimeState.repair`. A `RepairPlanned` decision is
  recorded when the runbook contains compensation, manual work or errors.
- `master repair --trace-id ... --state-root ... [--json]` prints the runbook
  and the recorded side effects. `master status` shows per-task
  `compensate/manual/errors` counts.
- Two-step scenario: step 1 verifies and records an effect; step 2 fails
  verification and gets a repair runbook that compensates step 1's artifact.

## Tests

- `failed_second_step_builds_compensation_for_the_first`: task `a` verified,
  artifact effect recorded; task `b` missing its artifact is rejected; `b`'s
  runbook compensates `a.txt`; a `RepairPlanned` decision names `b`.
- `lease_receipt_and_verification_unlock_dependencies` now also asserts the
  recorded effect and `repair_runbook().compensate == 1`.
- `cli_master_runs_verifies_and_replays_a_workshop_work_item` runs the real CLI
  through init/approve/run/verify and then `master repair --json`, asserting
  `compensate == 1`, the `result.txt` compensation, and `status == applied`.

Full gates (`gate.txt`, `eval.txt`):

- lib 1,348 passed / 0 failed / 4 ignored;
- bin 67 passed;
- CLI e2e 39 passed with the one pre-existing baseline failure skipped
  (`cli_tui_agents_refresh_uses_configured_profiles`, also fails on unmodified
  HEAD `264fa34`);
- MAST fault injection 7 passed;
- eval `evals/core.json` all scenarios PASS;
- strict `cargo clippy --offline --all-targets -- -D warnings` clean;
- `cargo fmt --check` clean.

## What this does not prove

- The runbook is a plan, not execution. Nothing is deleted automatically.
- Artifact ownership is not tracked. A compensation says to remove the file
  only if this task created it; otherwise the operator must restore the previous
  content. Pre-write snapshots would make this fully reversible.
- Subagent child writes are not turned into compensation plans; their journals
  remain the source of truth. Scanning child journals and deleting files would
  be unsafe without pre-write content.
- A blocked/failed task with no applied artifacts gets no repair runbook
  (`repair` stays `None`), so the status output is not noisy.
- The full `GovernedTaskGraph` policy path (ticket, mandate, approval, action
  log) is not duplicated into Master. Master's own approval/lease/verification
  state is the governing record for this slice.

## Release provenance

Feature commit and release binary are recorded in the follow-up provenance
commit (`release.txt`, `sha256.txt`).
