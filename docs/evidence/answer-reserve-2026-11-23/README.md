# Budget-aware acceptance reserve

Status: complete. Full product eval **24/24**, pass^3 **8/8**, pass at least
once 8/8; bug-fix-last-index and long-task-plan are both 3/3.

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
- A write after an accepted receipt sets `receipt_invalidated`; the reserve
  then advertises `verify_artifact` and `acceptance_gate` and asks for a fresh
  receipt for the new bytes.
- The reserve step may spend one provider attempt beyond the configured
  `max_provider_attempts`; the tool-free budget answer keeps its existing
  reserved attempt.
- A completed delivery with no flip condition gets one runtime flip condition;
  the first reserve run made bug-fix `completed` but left 0 flips and failed
  its rubric, which surfaced this gap.
- `StepModelMetrics.max_steps` still reports the configured budget, not the
  internal reserve.
- Tests: `step_budget_reserves_one_acceptance_turn` (max_steps=2: write, verify,
  reserved acceptance gate, then a completed budget answer; four model calls),
  `write_after_receipt_gets_a_verify_and_regate_reserve`, and
  `mixed_budget_batch_runs_only_the_acceptance_gate`.

## Result

`product-eval-before-mixed-batch/` (message fix + step reserve + flip backfill):
22/24, pass^3 6/8. bug-fix 2/3 (one gate batched with a terminal call and the
whole batch was rejected at the tool budget), long-task 2/3 (the model wrote
`NOTES.md` after an accepted receipt; the completion gate correctly downgraded
it). The mixed-batch rescue then fixed the bug-fix path.

`product-eval-before-invalidation/` (mixed-batch rescue, before the post-write
reserve): 23/24, pass^3 7/8; bug-fix 3/3, long-task 2/3 (one run wrote
`NOTES.md` after the accepted receipt and the gate correctly downgraded it).

Final full product eval N=3: **24/24 = 100%, pass^3 8/8, pass at least once
8/8**. bug-fix-last-index and long-task-plan are both 3/3; every accepted
delivery kept a valid receipt and at least one flip condition.

## Gates

- fmt, strict clippy, lib 1447/0/4, bin 71/0, core eval 8/8.
- Binary `ef3f0d69…`; full eval N=3 uses the same binary.

## Limits

- The reserved step filters the advertised tools (acceptance gate, plus
  verify_artifact after a receipt-invalidating write) but does not hard-block an
  invented call; an invented write can only clear the receipt, never produce
  `completed`.
- If the model ignores the reserve nudge, the run still ends conditional; the
  suite now passes 3/3 on this task, but the behavior remains model-dependent.
- N=3 × 8 tasks is directional, not a broad quality claim.
