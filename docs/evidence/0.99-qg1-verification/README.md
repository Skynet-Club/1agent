# 0.99 QG1 remediation — goal closure verification evidence

Date: 2026-11-17  
Tree: `/home/skynet/1-agent`, branch `0.99-dev`  
Base HEAD: `29d657d`; remediation commit: `c0fcaa1`  
Related: ADR-0147, `docs/evidence/0.99-acceptance/README.md` §3 QG1

## 1. Finding being fixed

The independent 0.99 QA found that `goal close --closure achieved` trusted a
caller-supplied `--verification passed`:

- `src/main.rs` mapped the string directly to `GoalVerification::Passed`;
- `GoalGraph::close_with_evidence` checked only the enum, a linked run id and
  pending commitments;
- `tests/goal_cli.rs` linked a synthetic `run-1` with no journal and still
  closed the goal as achieved.

That violated the plan requirement that closure cite an answer and a
verification record, not only an assertion.

## 2. Implementation

- `GoalOutcome` now carries optional `GoalVerificationEvidence`
  (`verification_id`, `run_id`, `executor_id`, `verifier_id`,
  `verified_at_ms`).
- `close_with_evidence` for `achieved` requires the evidence, requires
  `evidence.run_id == answer_run_id`, rejects empty ids and
  `executor_id == verifier_id`, and stores the evidence in the hash-chained
  `GoalStatusChanged` event.
- `1agent goal close --closure achieved` now requires
  `--verification-id` and reads the record from
  `--verification-store` (default `~/.1a/verification.jsonl`). It rejects:
  missing record, record for another run, `Rejected`, and `Hacked`.
  `--verification passed` alone is no longer sufficient.
- Old goal logs and `GoalSnapshot`s without the new optional field still
  replay.

## 3. Verification

| Command | Result |
|---|---|
| `cargo test --offline --test goal_cli` | **5 passed / 0 failed** |
| `cargo test --offline --lib goal` | **27 passed / 0 failed** (1168 filtered out) |
| `cargo clippy --offline --all-targets -- -D warnings` | **passed** |
| `./scripts/verify.sh` | **passed**, 1632 total / 0 failed / 13 ignored, core-v3 8/8 |

Raw log: `verify-2026-11-17.log`.

Covered negative cases:

- `achieved` without a verification record fails (`MissingVerification`);
- a record for another run fails (`VerificationRunMismatch`);
- a self-verified record (`executor_id == verifier_id`) fails
  (`InvalidVerificationEvidence`);
- CLI rejects a missing record, a wrong-run record, `Rejected` and `Hacked`;
- a valid `Verified` record for the linked run closes the goal and the
  verification id is persisted in the goal event.

Old-log coverage: the existing `legacy_goal_snapshot_without_bindings_is_readable`
test and the full goal replay suite remain green.

## 4. Explicit non-claims

- The verification store is local JSONL. It enforces an executor/verifier
  identity split and a `Verified` outcome, but it is not a signed approval
  envelope and does not prove the verifier's real-world independence.
- The closure binds a verification record to a run id; it does not read or
  hash the answer text. Answer inspection remains a journal/session-export
  operation.
- This slice does not change Track Q's 95/100 result or the other five
  acceptance failures. It only closes the QG1 closure-evidence gap.
- The change is a deliberate CLI tightening: `--verification passed` without
  `--verification-id` now fails for `achieved`.
