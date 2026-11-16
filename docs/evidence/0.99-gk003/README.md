# 0.99 GK-003 — close goal with evidence, evidence

Date: 2026-11-16
Scope: `GoalOutcome` / `GoalClosure` / `GoalVerification` and
`close_goal_with_evidence` in the existing hash-chained goal log.
Result: **implemented and verified; GoalKernel is still not accepted as a
whole.** This is not a release.

All fixtures are synthetic and live in temporary directories. No production
journals, prompts, credentials or secrets are included.

## 1. Tested tree

| Field | Value |
|---|---|
| Repository | `/home/skynet/1-agent` |
| Branch | `0.99-dev` |
| Slice tag | `0.99-gk003-close-evidence` |
| Changed files | `src/goal.rs`, `src/surface_service.rs`, `docs/architecture.md`, `docs/0.99-handoff.md` |

## 2. Commands and results

```bash
cargo test --offline --lib
./scripts/verify.sh
```

| Command | Result |
|---|---|
| `cargo test --offline --lib` | **1092 passed, 0 failed, 4 ignored** |
| `./scripts/verify.sh` | **passed** (fmt, clippy all-targets `-D warnings`, all tests, core-v3 8/8) |

Focused tests added:

- `goal::tests::close_with_evidence_achieves_only_with_a_linked_run_and_passed_verification`
  — positive closure, replay equality, second close fails.
- `goal::tests::close_with_evidence_rejects_unproven_achieved` — missing run,
  unlinked run, failed verification, open commitment, unknown/foreign
  commitment, empty reason; nothing is persisted on failure.
- `goal::tests::abandoned_closure_requires_reason_and_cancels_every_pending_commitment`
  — unlisted pending commitment fails; listed pending commitments are
  cancelled with the reason; paused → abandoned is allowed; replay equality.
- `goal::tests::store_level_close_helper_persists_and_replays` — the store
  helper persists the closure and replays after reopen.

## 3. Acceptance mapping

| GK-003 criterion | Evidence |
|---|---|
| `achieved` needs a completed run and passed verification | Requires `answer_run_id` linked to the goal and `GoalVerification::Passed`; each failure mode has a test |
| Otherwise only paused/abandoned | `achieved` is rejected for non-active goals; `abandoned` is explicit and tested from active and paused states |
| `abandoned` records a reason and closes open commitments | Reason is required (1..=240 chars); every pending commitment must be enumerated and is cancelled in the same candidate graph |
| Closing keeps history and a new goal needs a new id | The log is append-only; replay tests compare snapshots; no deletion path exists |
| No-evidence closure fails, evidence closure succeeds | Negative and positive tests above; failed calls append nothing |

## 4. Explicit non-claims

- The function trusts the caller's `GoalVerification::Passed`; it does not
  re-run verification or read the answer run journal. A stronger verifier
  binding is future work.
- TUI/CLI wiring for closing goals is not part of this slice; `1agent goal
  close` is GK-005.
- `GoalBinding` ontology nodes and CQ answers remain GK-006.
- GoalKernel is not accepted: GK-004 commitment lifecycle negatives and the
  remaining G1 checks are still open.
