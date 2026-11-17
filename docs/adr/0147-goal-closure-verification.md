# ADR 0147: Goal Closure Requires a Stored Verification Record

- Status: accepted
- Date: 2026-11-17
- Related: ADR-0014, ADR-0037, `src/goal.rs`, `src/verification_store.rs`,
  `src/main.rs`, `tests/goal_cli.rs`

## Context

Independent QA of 0.99 found that `goal close --closure achieved` trusted a
caller-supplied `--verification passed`. `GoalGraph::close_with_evidence`
checked only the enum, a linked run id and pending commitments. It never read
the linked run journal, the answer, or a verification record.

The existing CLI test demonstrated the gap: it linked a synthetic `run-1` for
which no journal existed and still closed the goal as achieved. This violates
the 0.99 plan requirement that closure cite an answer and a verification
record, not just a model or caller assertion.

The repository already has a separate `VerificationStore` with records that
bind a verification id, a run id, an independent verifier and a
`Verified`/`Rejected`/`Hacked` outcome.

## Decision

1. `GoalOutcome` carries an optional `GoalVerificationEvidence` containing
   `verification_id`, `run_id`, `executor_id`, `verifier_id` and
   `verified_at_ms`.
2. An `achieved` closure requires that evidence. The run id in the record must
   equal `answer_run_id`; verification and executor ids must be non-empty and
   distinct; the linked run must still belong to the goal; and pending
   commitments must be closed.
3. `GoalStatusChanged` stores the evidence in the hash-chained goal log. Replay
   does not need the verification store, and old events without the optional
   field remain readable.
4. `1agent goal close --closure achieved` requires `--verification-id` and
   reads the record from `--verification-store` (default
   `~/.1a/verification.jsonl`). It rejects a record for another run and any
   outcome other than `Verified`. The `--verification` flag can no longer
   substitute for a record.
5. `abandoned` keeps its existing reason and pending-commitment cancellation
   rules and does not require verification evidence.

## Consequences

- A goal can no longer be achieved from a bare flag or a nonexistent run. A
  missing, mismatched, self-verified, rejected or hacked record fails closed.
- Existing callers that use the library API must provide evidence; existing
  goal logs and snapshots still replay.
- The CLI is intentionally stricter: `--verification passed` without
  `--verification-id` now fails for `achieved`.
- The verification store is local JSONL. It records verifier identity and
  executor/verifier separation, but it is not a signed approval envelope and
  does not prove the verifier's real-world independence. That remains a
  separate trust and audit concern.
- The closure binds a verification record and run id, not the answer text.
  Querying the answer remains a journal/session-export operation.

## Alternatives considered

- **Read the run journal directly in `goal close`.** It would prove the run has
  an `AnswerProduced` record, but would duplicate journal parsing in the goal
  domain and still not establish independent verification.
- **Keep the enum and add a boolean.** A boolean is still caller assertion.
- **Store the full verification report in the goal log.** It duplicates a
  large record; the evidence ids plus the hash-chained goal log are sufficient
  for replay and inspection.
