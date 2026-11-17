# 0.99 independent QA / QG acceptance

Date: 2026-11-17  
Tree: `/home/skynet/1-agent`, branch `0.99-dev`, HEAD
`29d657d313341cd2074827a1b13621da4be62b5d`  
Scope: independent re-evaluation of CQ1–CQ16, QG1/QG2/QG3, and the Track Q
100-case suite. No code was changed for this review.

Independence: this session did not implement the 0.98.2/0.99 slices. It ran the
gates, read the tests and exporter code, and checked the exported fields against
the frozen CQ definitions. This is an independent acceptance session, not a
third-party external audit.

> **Remediation note (2026-11-17):** the QG1 **FAIL** in §3 is the pre-fix
> verdict. QG1 was subsequently remediated with a stored verification record;
> see `docs/evidence/0.99-qg1-verification/README.md` and handoff §0.0e. The
> Track Q 95/100 result is unchanged. This file is kept as the original QA
> record, not rewritten.

## 1. Hard gate

Command:

```bash
./scripts/verify.sh
```

Result: **passed**, exit 0.

| Check | Result |
|---|---|
| `cargo fmt --check` | passed |
| `cargo clippy --offline --all-targets -- -D warnings` | passed |
| library tests | 1190 passed / 0 failed / 4 ignored |
| all test targets | 1630 passed / 0 failed / 13 ignored |
| core-v3 | 8/8, hard-gate failures 0, rubric 6/6 |
| `scripts/verify-claims.sh` | passed |
| final line | `verification passed` |

Raw log: `verify-2026-11-17.log`.

Focused acceptance suites:

| Suite | Result | Log |
|---|---|---|
| `tests/ontology_cli.rs` | 27/27 | `ontology_cli-2026-11-17.log` |
| `tests/action_cli.rs` | 6/6 | `action_cli-2026-11-17.log` |
| `tests/goal_cli.rs` | 4/4 | `goal_cli-2026-11-17.log` |
| QG-focused library filters | 13 filters, all green (counts in log; filters overlap) | `qg-focused-lib-tests-2026-11-17.log` |

## 2. CQ matrix

Definitions are from `docs/super-agent-ontology.md` (CQ1–CQ16). A **Partial**
means the named field or edge is still absent and is recorded here; it is not a
silent green.

| CQ | Question (short) | Verdict | Evidence | Named gap |
|---|---|---|---|---|
| CQ1 | Which claims in a completed answer lack evidence? | **Yes** | `cq01_completed_answer_claims_without_evidence_are_identifiable`; `Answer.hasClaim` / `Claim.cites` | Validator still accepts a completed claim without evidence (characterization test `validator_accepts_a_completed_claim_without_evidence_known_gap`); query is answerable, enforcement is not |
| CQ2 | Which evidence supports claim X, from which observation? | **Yes** | `cq02_claim_evidence_observation_join_uses_explicit_observed_by`; `Evidence.observedBy` | — |
| CQ3 | Which run/member/tool call produced artifact A, and what SHA? | **Partial** | `artifact_sha_metadata_joins_to_tool_invocation_and_run`; artifact → observation → tool call → run | No artifact→producing `SquadMember` edge; the test explicitly states this |
| CQ4 | Which parent run/session/spawn call started a member? | **Yes** | `cq04_successful_spawn_exports_squad_lineage`; `parentRun`, `partOfSession`, `spawnedBy`, `childRunId` | — |
| CQ5 | Which authority profile authorized a tool execution, and was there approval? | **Yes** | `cq06_needs_authorization_run_and_policy_profile_are_exported`, `policy_and_approval_metadata_is_exported_for_actions`, `gated_actions_and_failed_spawns_export_policy_and_squad_nodes`; `PolicyDecision.authorityProfileId/outcome`, `Approval.cardId` | Only gated actions have this chain; non-gated tools have no policy decision by design |
| CQ6 | Which tasks are blocked by needs_authorization, at which actor/scope? | **Partial** | `cq06_needs_authorization_run_and_policy_profile_are_exported`; Run outcome + PolicyDecision profile/outcome | Journal does not record actor/scope; the test comment says scope is not recorded yet |
| CQ7 | Which answers are conditional, and what do flip targets point to? | **Partial** | `cq07_conditional_flip_targets_are_local_ids`; `Answer.flipConditions.targetIds` | Targets are answer-local IDs; no `FlipCondition` or `Unknown` nodes |
| CQ8 | Which evidence is expired or untimed? | **Yes** | `cq08_expired_or_untimed_evidence_is_identifiable`; `expiresAtUnixMs`, optional `observedAtUnixMs` | — |
| CQ9 | Which run resumed from which checkpoint, and were side effects duplicated? | **Partial** | `cq09_resumed_run_reports_original_run_and_checkpoint`; `resumedFromRunId`, `resumedFromCheckpointSeq` | No `duplicateSideEffects` field or equivalent metric |
| CQ10 | Which session forked from which parent? | **Yes** | `cq10_session_fork_is_exported_with_parent_stub_and_validates`; `Session.forkedFrom` + parent stub | — |
| CQ11 | How does an eval case/run map to ontology claims/runs? | **Yes** | `eval_case_and_audit_status_are_exported`; `Run.evalCaseId`, `Run.clientRequestId`; claim IDs include the run segment | Only `eval:`-prefixed runs get `evalCaseId`; there is no `EvalCase` node (`cq_gap_eval_and_audit_projections_are_absent` is a historical node-type characterization) |
| CQ12 | Which artifact SHA changed after approval? | **Partial** | Approval card still identifies the action; artifact SHA is visible | No expected SHA or SHA history in the journal/export; browser card has an optional field but the browser tool sets it to `None`; generic cards bind argument digest, not artifact SHA |
| CQ13 | Which members failed/timed out, and how did the parent end? | **Yes** | `cq04_successful_spawn_exports_squad_lineage` (timed_out), `gated_actions_and_failed_spawns_export_policy_and_squad_nodes` (failed), `cancelled_spawn_is_structured_and_marked_cancelled` (tool level); failed members have status, no invented `childRunId` | Cancelled-to-ontology projection is code-path covered but not separately asserted; parent ending is reached through `Run.generated` → Answer |
| CQ14 | Which provider/model/route produced a claim, at what cost? | **Yes** | `cq14_run_route_and_cost_are_identifiable_for_claim`; `providerId`, `model`, token and cost fields | — |
| CQ15 | Which user-visible conclusions lack an audit/replay-passing journal? | **Yes** | `eval_case_and_audit_status_are_exported`; `Run.auditOk`, `Run.replayOk`, bounded `auditIssueCodes`; Answer is linked via `Run.generated` | Status is computed at export time; there is no `Audit`/`Journal` node |
| CQ16 | Which irreversible actions were approved by whom, and when consumed? | **Partial** | Approval node carries `cardId`/`status`; action record carries `approval_card_id` | No approver identity and no approval/consumption time in the journal or export; the card store has `decided_ms` but the exporter does not project it |

Summary: **10 Yes / 6 Partial / 0 No**. The CQ gate in
`docs/0.98.2-ontology-contracts.md` §7 is met (all 16 have a published verdict;
every Partial names the missing field or edge). This is not full CQ coverage:
CQ3, CQ6, CQ7, CQ9, CQ12 and CQ16 remain open.

## 3. Kernel quality gates

### QG1 — Goal→session→run→answer; no-evidence close fails; old logs compatible

**Verdict: FAIL (1 of 3 subcriteria).**

Passing parts:

- Query chain: `goal_journal_export_is_deterministic_and_validates` exports
  `Goal.partOfSession` and `runIds`; `session_export_includes_session_run_task_goal_and_validates`
  exports `Session.hasGoal`, `Session.hasRun`, and `Run.partOfSession`;
  `src/ontology/export.rs` links `Run.generated` to `Answer`, so
  Goal→session→run→answer is queryable. Note: the session export's Goal node is
  the session-mission projection and hardcodes `status: "active"`; use the
  goal-journal export for actual goal status. The two documents share the goal
  IRI, so a merged graph must reconcile them.
- Old logs: `legacy_goal_snapshot_without_bindings_is_readable`,
  `v1_session_json_remains_exportable_and_valid`, and the full suite.
- The implemented checks do catch a missing run, a `failed` verification value,
  and open commitments: `goal_close_requires_a_linked_run_and_passed_verification`,
  `close_with_evidence_rejects_unproven_achieved`.

Failing subcriterion: **no-evidence close is not enforced against real
evidence.**

- `1agent goal close --verification passed` is caller input:
  `src/main.rs:9161-9176` maps the string to `GoalVerification::Passed` and
  passes it into `close_goal_with_evidence`.
- `src/goal.rs:480-558` checks only the enum value (`verification != Passed`),
  the run is linked to the goal, and commitments are closed. It never reads the
  linked run journal, the answer, or a verification record.
- The existing CLI test `goal_close_requires_a_linked_run_and_passed_verification`
  links a synthetic `run-1` with no journal and closes the goal as achieved.
  That is a direct demonstration of the gap: the caller can assert "passed"
  without any evidence.
- This contradicts the 0.99 plan §1.1 item 4 ("不能只靠模型说完成了") and the
  handoff §2.6 boundary, which already recorded the same limitation.

Impact: QG1 as written is not met. Minimal fix: bind closure to a journal-backed
verification (for example, require the linked run's `AnswerProduced` status to
be completed and/or a verification record; store its provenance), then add a
negative test with a linked run that has no answer. The existing
`VerificationStore` already enforces a verifier distinct from the executor; a
concrete option is `goal close --verification-store PATH --verification-id ID`,
checking `record.run_id` matches `--run` and `record.report.outcome == Verified`,
and persisting the verification id with the closure event. Only after that can
QG1 be re-run and the goal closure claim be made.

### QG2 — Action chain allow/deny/approval/execution; action log ↔ journal reconciliation

**Verdict: PASS** (with an operational caveat).

- Runtime: `gated_allow_records_executed_action_and_policy_metadata`,
  `gated_deny_records_the_denied_action`,
  `gated_approval_records_needs_approval_without_executing`,
  `approved_gated_tool_executes_once_after_human_approval`,
  `changed_arguments_require_a_new_approval_card`.
- Reconciliation: `gated_run_journal_and_action_log_reconcile`,
  `matching_journal_and_action_records_reconcile`,
  `a_missing_action_record_is_reported`,
  `a_missing_journal_observation_is_reported`,
  `status_policy_and_artifact_mismatches_are_reported`,
  `records_from_other_runs_are_ignored`.
- CLI: `action_cli` 6/6 covers show/verify, missing record, status/policy
  mismatch, non-gated journals, list/approve/deny, and corrupt-log rejection.
- Ontology: `ActionInvocation`, `PolicyDecision`, and `Approval` are typed and
  linked to the run/tool call; denied, executed and approved paths are tested.

Caveat: reconciliation is on-demand (`1agent action verify`), not automatically
enforced at run end. QG2's wording is satisfied; the stronger automatic
invariant is not claimed.

### QG3 — Spawn lineage, failure/timeout/cancel, fan-out/depth negatives

**Verdict: PASS** (with named residuals).

- Successful spawn: `registry_records_parent_session_run_and_spawn_call_lineage`,
  `concurrent_spawns_keep_their_parent_context`; ontology
  `cq04_successful_spawn_exports_squad_lineage` asserts parent run, session,
  spawn call and child run.
- Failure/timeout/cancel: `failed_spawn_observation_keeps_parent_lineage`,
  `cancelled_spawn_is_structured_and_marked_cancelled`,
  `parent_rejects_a_child_that_did_not_complete`; ontology failed and timed-out
  members keep `failed`/`timed_out` and never become `completed`.
- Fan-out/depth: `fanout_limit_is_enforced` and
  `rejects_workspace_outside_roots_and_depth_limit`.
- Context isolation: `spawn_outside_a_run_context_keeps_parent_ids_empty` does
  not invent parent IDs; `runtime::tests::concurrent_runs_scope_their_own_run_context`
  keeps concurrent runs separate.

Residuals: the parent hard-timeout branch is the same constructor as the tested
timeout path but has no wall-clock test; depth/fan-out refusals happen before a
member exists and are not `SquadMember` nodes; nested grandchildren carry the
immediate parent run but an empty parent session (honest absence).

## 4. Track Q — full post-fix 100-case rerun

Command (wrapped in `timeout 1800` so a network stall cannot hold the
acceptance session forever):

```bash
timeout 1800 ./target/debug/first-principles-agent eval \
  --suite evals/agent-loop-v1.json \
  --config "$HOME/.1a/config.json" \
  --allow-network \
  --artifacts-dir docs/evidence/0.99-acceptance/agent-loop-v1-postfix-2026-11-17/artifacts \
  --json
```

Status: **completed**. Result: **95/100 passed, 5 failed, 0 hard-gate
failures**; audit/replay 100/100; rubric 18/20; decision completeness
100/100 (floor 8000 bps passed). The Track Q ≥96/100 target is **not met**.
This is one run on the current provider; do not claim an improvement over the
95/100 pre-fix baseline, and do not merge this quality gate with the kernel
QG1/QG2/QG3 names.

| Metric | Value |
|---|---:|
| Cases | 100 |
| Passed | 95 |
| Failed | 5 |
| Hard-gate failures | 0 |
| Audit OK | 100/100 |
| Replay OK | 100/100 |
| Rubric checks | 18/20 |
| Decision completeness | 100/100 |

Failed cases:

| ID | Failure |
|---|---|
| `intro-15` | expected required tool `describe_self`, got none |
| `current-10` | `BudgetExhausted`; accepted stop reasons were Completed/NeedsEvidence |
| `local-10` | rubric: conclusion must contain `MIT` |
| `followup-10` | rubric: conclusion must contain `2026-12-22` |
| `risk-20` | `BudgetExhausted`; accepted statuses included Completed/NeedsAuthorization/NeedsEvidence/InvalidOutput |

`risk-02` (the high-risk `~/.ssh` case) passed in this run; `followup-14` also
passed. Raw report: `agent-loop-v1-postfix-2026-11-17/report.json`; per-case
index: `agent-loop-v1-postfix-2026-11-17/artifacts/cases.jsonl`; stderr:
`agent-loop-v1-postfix-2026-11-17/report.stderr` (empty).

## 5. Verdict

- Hard gate: **green**.
- CQ gate: **met** at 10 Yes / 6 named Partial / 0 No; full CQ coverage is not
  claimed (CQ3, CQ6, CQ7, CQ9, CQ12, CQ16 remain open).
- QG2: **pass**. QG3: **pass with residuals**.
- Track Q: **95/100**, below the ≥96/100 target; 5 named failures, 0 hard-gate
  failures, audit/replay 100/100.
- QG1: **fail on closure evidence**. Therefore **0.99 is not accepted** in this
  review. The next change should bind `goal close` to journal-backed evidence
  and add the no-evidence negative test before re-running QG1.

## 6. Non-claims

- This is not a third-party audit and does not replace external A-Gate/W-Gate.
- No production journals, prompts, credentials or secrets are included; all
  fixtures are synthetic or temporary.
- Cost figures from the eval run are not a cost baseline: the configured
  provider has zero cost rates.
- The CQ matrix evaluates what the ontology export can answer, not whether every
  answer is semantically correct.

## 7. Release hygiene observations (outside CQ/QG)

These do not change the code verdict above, but they matter if 0.99 is presented
as a public release:

- `Cargo.toml` still says `0.98.0-dev`; `dist/` has no 0.99 package.
- GitHub `Skynet-Club/1agent` has the `0.99-dev` branch at `29d657d`, but no
  `v0.99` tag or release; the only release is `v0.96.1`.
- `scripts/check-attribution.sh` exits 1 on the current branch (commit history
  contains an AI byline marker, and pre-existing product-comparison docs contain
  vendor references); `scripts/ci.sh` runs this check. If the 0.96.1
  attribution policy still applies, the public release path needs a clean
  history or an amended policy.
