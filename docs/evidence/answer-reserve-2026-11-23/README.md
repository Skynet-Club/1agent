# Budget-aware acceptance reserve

Status: complete. Full product eval **23/24**, pass^3 **7/8**, pass at least
once 8/8; bug-fix-last-index is 3/3.

- `src/runtime.rs`: Act runs get one internal step beyond `max_steps`, used only
  when side-effect tools have run, a verified artifact exists, and no accepted
  receipt does. That step advertises only `acceptance_gate` and adds an internal
  acceptance nudge. When the conditions are absent the loop breaks before the
  model is called, so normal runs spend nothing.
- The step-budget answer now builds its tool-free request from the latest
  messages, so it sees the last tool result (for example an accepted receipt)
  instead of the request that produced the last tool call.
- At the tool budget, a mixed batch that contains `acceptance_gate` runs the
  gate alone; the other calls are journaled as paired `budget_skipped`
  observations. This covers the model batching a compile command with the gate.
- A completed delivery with no flip condition gets one runtime flip condition;
  the first reserve run made bug-fix `completed` but left 0 flips and failed
  its rubric, which surfaced this gap.
- `StepModelMetrics.max_steps` still reports the configured budget, not the
  internal reserve.
- Test: `step_budget_reserves_one_acceptance_turn` (max_steps=2: write, verify,
  reserved acceptance gate, then a completed budget answer; four model calls).

## Result

`product-eval-before-mixed-batch/` (message fix + step reserve + flip backfill):
22/24, pass^3 6/8. bug-fix 2/3 (one gate batched with a terminal call and the
whole batch was rejected at the tool budget), long-task 2/3 (the model wrote
`NOTES.md` after an accepted receipt; the completion gate correctly downgraded
it). The mixed-batch rescue then fixed the bug-fix path.

Final full product eval N=3: **23/24 = 95.83%, pass^3 7/8, pass at least
once 8/8**. bug-fix-last-index is now 3/3; long-task-plan is 2/3 (repeat 2
wrote `NOTES.md` after the accepted receipt, so the gate correctly downgraded
it to `conditional`).

## Limits

- The reserved step filters the advertised tools to `acceptance_gate` but does
  not hard-block an invented call; an invented write can only clear the
  receipt, never produce `completed`.
- A write after an accepted receipt still invalidates it; the long-task flake
  wrote `NOTES.md` after acceptance and was correctly downgraded. Re-verifying
  and re-gating after such a write remains model behavior, not a runtime
  rescue.
