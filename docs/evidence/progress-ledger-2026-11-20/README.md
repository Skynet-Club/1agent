# MA-006 progress ledger / stall counter (2026-11-20)

Status: **bounded progress ledger and stall detection landed**

## What changed

Every `TaskRuntimeState` now carries a `ProgressLedger`:

```json
{
  "last_action": "blocked: waiting for input",
  "last_signature": "blocked:waiting for input",
  "stall_count": 2,
  "repeated_actions": 2,
  "last_progress_unix_ms": 1795...,
  "last_observed_unix_ms": 1795...
}
```

Behavior:

- `Blocked` and `AwaitingInput` observations get a signature from their
  reason. The first occurrence is progress; each identical repeat increments
  `stall_count` and `repeated_actions`.
- A different reason, a `Running` heartbeat, a submission, or a finished
  receipt resets the stall count and records progress time.
- At `STALL_THRESHOLD = 3` repeated no-progress observations, the task moves
  to `Replanning` (via `Blocked` when needed) and `record_observation` returns
  `MasterCoordinationError::Stalled(task_id)`.
- The ledger is part of `TaskRuntimeState`, so it is persisted in the Master
  snapshot and visible in `master status --json`.

## Why this shape

This follows the Magentic-One Task/Progress Ledger idea without a heavy
step-level reward model: track recent action and no-progress repeats, and
replan when the same blocked state repeats. Running heartbeats are deliberately
ignored for stall counting to avoid false positives.

## Tests

- `cargo test --lib`: **1323 passed / 0 failed / 4 ignored**.
- `master_coordination` 15 tests, including:
  - repeated `Blocked` with the same reason → Replanning + `Stalled`;
  - a changed reason resets the counter;
  - repeated `Running` heartbeats do not stall.
- `cargo clippy --lib`: no warnings.

Installed binary: `~/.local/bin/1agent` sha256 `aa6c9c8a…`; previous
`550ca10a…` backed up as `1agent-pre-progress-ledger-2026-11-20`.

## What this proves

- A stalled worker that keeps reporting the same blocked/awaiting-input reason
  is detected rather than silently retried forever.
- Progress is reset by real changes, so a live worker is not punished for
  heartbeats.

## What this does not prove

- A task that stays `Running` but never advances is not detected; that needs a
  lease heartbeat/deadline (MA-005) or a worker-reported progress field.
- `Replanning` is recorded, but automatic replan execution is still owned by
  the caller; no new journal event kind was added for the stall decision.
- One task-level test path; no multi-agent fault injection yet.

## Next

1. MAST fault injection: fake worker, repeated blocked action, fake SHA,
   self-verification.
2. Clarification gate before decomposition.
3. MA-005 lease heartbeat/reaper/fencing.
