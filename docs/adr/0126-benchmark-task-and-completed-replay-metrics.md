# ADR 0126: Task-Level M1 and Completed-Run M5 Metrics

- Status: accepted
- Date: 2026-11-13
- Related: M1, M5, M13, ADR-0108, ADR-0115

## Context

`benchmark-run` treated one whole-suite iteration as one sample:

- M1 completion was “all cases in the suite passed”, which compounds per-task
  reliability and is not the M1 definition (“tasks with a verified artifact that
  passes acceptance”);
- `replay_verified` was computed from report integrity plus `audit_ok`, not from
  replaying the journal, and counted failed as well as completed cases.

The 8h result (136/280 suites, 48.57%) therefore could not be compared directly
with the 0.97 M1 target, and its 85% “replay” number was not M5.

## Decision

1. `EvalCaseResult` gains `replay_ok`, computed with `replay_events` on the
   case journal; old reports deserialize it as `false`.
2. `benchmark-run` accumulates task-level results across all iterations:
   `tasks_total`, `tasks_passed`, `task_completion_bps`, `completed_cases`,
   `completed_replay_ok` and `completed_replay_bps`.
3. A case counts as completed for M5 only when `stop_reason` is `Completed`.
   The M5 numerator also requires both `audit_ok` and `replay_ok`.
4. New flags `--min-task-completion-bps` (M1) and
   `--min-completed-replay-bps` (M5) are added. They default to 0 (disabled), so
   existing callers keep the historical suite gates. The suite-level
   `--min-completion-bps` and `--min-replay-bps` remain for compatibility and
   are labelled as historical suite gates.
5. Suite `replay_verified` now requires `audit_ok && replay_ok` on every case,
   so the field no longer claims replay when only audit was checked.

## Consequences

- The existing 8h JSON has no task-level counts and cannot be retroactively
  scored as M1/M5; a new benchmark run is required.
- M1 is now comparable with the 0.97 ≥50% and 1.0 ≥70% targets at the task
  level. M5 excludes failed model outputs from its denominator, as its
  definition requires.
- A suite with an invalid case still fails the historical suite
  completion/replay gates; the new task-level fields expose the distinction.
