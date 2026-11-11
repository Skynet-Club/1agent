# 1A Master-Agent V1

Status: implemented and live-tested against Skynet Workshop 0.11.0

## Boundary

1A owns task triage, planning, Agent selection, bounded supervision, independent
verification and audit. Skynet Workshop owns Session/Pane/Job creation, execution,
capacity, controls and platform events.

```text
user goal
  -> deterministic triage
  -> model-proposed PlanProposal
  -> deterministic plan validation and approval
  -> registered capability filtering
  -> task lease with idempotency key
  -> Workshop agent-run / Session
  -> ExecutionReceipt
  -> independent artifact and criterion verification
  -> verified dependency unlock
  -> bounded retry or scoped replan
  -> Master Journal replay and audit
```

A Work Item is never identified by a Pane or Session. A Session is one execution
target recorded in an `ExecutionReceipt`. A task can be retried with a new lease
generation without changing its Work Item identity.

## Commands

Workshop 0.11.1 also exposes `1-Agent` inside the Master-Agent runtime picker. Selecting
it starts `1agent tui` in the persistent Coordinator Session, so the normal conversation
surface is 1A itself; planning and controlled execution remain available through the
`master` commands and the same journal/state contracts.

Standalone triage and planning do not require Workshop:

```bash
1agent master triage --task "先审计，然后修复，最后测试" --json

./scripts/1agent-pi master plan \
  --task "先创建 result.txt，然后验证内容" \
  --workspace /path/to/workspace \
  --write \
  --planner-journal-dir .master-plans \
  --output plan.json
```

Run the versioned cross-runtime capability probe:

```bash
1agent master capability-probe \
  --manifest configs/capability-probes.example.json \
  --output capability-report.json
```

Add `--runtime deepseek-1agent` to run only the independent 1A DeepSeek route after a
shared-suite pass. DeepSeek uses runtime-specific JSON-path rules for
`answer.conclusion`; its result is not conflated with Pi's runtime.

The probe executes configured runtime commands and can consume provider quota. It reports
only bounded byte counts and SHA-256 digests for stdout/stderr, plus normalized pass,
fail or blocked status. It does not store raw output or credentials. A `blocked` result is
not counted as a failed capability run.

Create an unapproved dry-run state, inspect it, then approve:

```bash
1agent master init \
  --plan plan.json \
  --agents configs/master-agents.example.json \
  --workshop-prefix skynet-workshop \
  --state-root .master

1agent master status --trace-id <trace> --state-root .master
1agent master approve --trace-id <trace> --state-root .master --actor user
1agent master accept --trace-id <trace> --state-root .master --reason "all tasks verified"
1agent master return --trace-id <trace> --state-root .master --task T1 --reason "operator done"
1agent master revise --trace-id <trace> --state-root .master --plan plan-v2.json --agents agents.json
1agent master reject --trace-id <trace> --state-root .master --reason "requirements changed"
```

Dispatch ready Work Items. `--no-gui` is useful for automation; omitting it asks
Workshop to open an attachable GUI:

```bash
1agent master run \
  --trace-id <trace> \
  --state-root .master \
  --runtime-profiles configs/master-agents.example.json \
  --workshop-prefix skynet-workshop
```

Asynchronous execution keeps the Master process available for supervision:

```bash
1agent master run --detach --trace-id <trace> --state-root .master \
  --runtime-profiles agents.json --workshop-prefix skynet-workshop

1agent master poll --trace-id <trace> --state-root .master --json
```

Each async Work Item owns a persistent Workshop Session and an execution output/exit
receipt under `<state-root>/<trace>/executions/`. `poll` maps platform state to Master
`running`, `awaiting_input`, `blocked` or a terminal receipt. Agent runtimes can emit one
of these explicit output markers:

```text
[1A:AWAITING_INPUT] <question>
[1A:RUNNING]
[1A:BLOCKED] <reason>
```

The observer reads only a bounded 64 KiB output tail and uses the last marker. It does not
send Agent logs to the planning model.

Inside a Workshop Master TUI Session, the same durable state is available as
`/board`, `/watch`, `/attention`, `/respond`, `/control` and `/timeline`.
`/respond <task> <message>` writes only to that task's bound Session and is accepted
for `awaiting_input` or `blocked` tasks. Control actions are journaled after explicit
state and bounded-counter validation, so an invalid retry or replan cannot terminate a
live Session as a side effect.

Artifact acceptance is explicit:

```text
/artifacts [task-id|all]
/verify <task-id> <verification.json>
/evidence [master|run]
/audit [master|run|all]
/report [master|run]
```

`/artifacts` rechecks the current workspace instead of trusting a prior receipt.
`/verify` requires the verifier identity to differ from the assigned execution Agent.
After dispatch, Master evidence/audit/report views bind Work Items, receipts, Artifacts,
criteria, verifier evidence and decisions.

`/cost` summarizes 1A provider responses in the active session. It reports real journaled
token counts, configured cost micro-units, provider/model groups and recent runs. It does
not include external Worker costs unless those executions write a 1A run journal.

Execution success is not acceptance. Verification input supplies criterion and
test evidence; 1A itself checks expected Artifact existence, regular-file type,
path containment and SHA-256:

```bash
1agent master verify \
  --trace-id <trace> \
  --state-root .master \
  --input verification.json

1agent master audit --trace-id <trace> --state-root .master --json
```

Supervision and recovery:

```bash
1agent master retry --trace-id <trace> --task <id> --reason "one bounded retry"
1agent master replan-request --trace-id <trace> --task <id> --reason "plan invalid"
1agent master replan-apply --trace-id <trace> --plan replacement.json --agents agents.json --reason "user approved"
1agent master takeover --trace-id <trace> --task <id> --reason "user is editing"
1agent master cancel --trace-id <trace> --task <id> --reason "no longer needed" \
  --workshop-prefix skynet-workshop
1agent master pause --trace-id <trace>
1agent master resume --trace-id <trace>
```

Bounded read access to Workshop:

```bash
1agent master agents --json
1agent master jobs --json
1agent master events --since-ms 1793630000000 --limit 50 --json
```

## Safety invariants

- At most 8 Work Items.
- Every Work Item has capability, tool, acceptance and Artifact requirements.
- Artifact paths are relative and cannot escape the Work Item workspace.
- A model cannot set `approved=true` or change plan/goal identity.
- An unapproved plan cannot lease or dispatch work.
- An unstarted plan can be rejected or replaced by a higher-version revision; the old plan
  remains in Journal history.
- A nonterminal lease is never replaced with a newer generation.
- One idempotency key is accepted once; duplicate receipts fail closed.
- An expired lease requires explicit cancellation or takeover rather than blind
  re-dispatch.
- Dependency unlock requires `verified`, not model-declared completion.
- Verification requires exit code 0, exact criterion coverage, independent
  evidence, expected files and optional pinned SHA-256 values.
- Automatic retries are limited to one per task; automatic replanning is limited
  to one per run and only resets the affected failed subgraph.
- `takeover` prevents background dispatch from silently replacing user work.
- Every planner attempt and every Master state transition is journaled.

## Capability evidence

The declared launch matrix is in `configs/master-agents.example.json`. Declared
capabilities are not measurements. The 2026-11-02 local probe found:

Machine-readable probe evidence is in
`docs/master-capability-matrix-v1.json`.

| Runtime | Version | Authentication / configuration | Same non-interactive task |
|---|---|---|---|
| Pi | 1.4.0 | DeepSeek auth reports `ready` | blocked by restricted execution network: `Connection error` |
| Hermes | 0.22.5 | DeepSeek credential configured | one-shot produced no final response within 60 s |
| Claude | 2.1.314 | OAuth login present | rejected current `deepseek-flash[1m]` model selection |
| Codex CLI | 0.166.0 | not logged in | not runnable; isolated app-server also hit read-only filesystem |

These results are deliberately not promoted to success rates. `observed_runs`
remains zero until a repeatable task suite completes in a writable, authenticated
runtime environment.

Workshop control-plane evidence collected from its own persistent Session:

- Registry read: 13 records, including live `main-coordinator`.
- Job read: 7 historical jobs; 4 `ok`, 1 `fail`, 2 `cancelled`.
- Unified event read: bounded structured events returned with no malformed records.

## Live acceptance

Natural-language planning also completed against the configured DeepSeek endpoint:

```text
plan_id: plan-0b58c6ef-5371-4473-9d67-ba7ed1fbeff8
Work Items: create-plan-demo -> verify-plan-demo
max_parallel: 1
approved: false
risk: high for the write-bearing Work Items
read_roots: constrained to the planning workspace
planner evidence: target/master-live/planner-journal/plan-0b58c6ef-5371-4473-9d67-ba7ed1fbeff8.jsonl
```

The release binary completed a real Workshop path on 2026-11-02:

```text
trace_id: trace-final-bounded-20261103
Work Item: create-artifact
Agent: live-shell-worker
Session: term-1793636352398-530889-18
Artifact: result.txt
SHA-256: 4db03012bc6c0d59c1d05739bb9c9ec592d33d62a3a190f768a0a4cd9e7606bc
Verification: verified
Journal: 7 events, audit ok
```

The state and hash-chained journal are under
`target/master-live/state/trace-final-bounded-20261103/`.

An additional async acceptance observed the real Session in `running` state, then polled
the terminal receipt, independently verified the Artifact and audited eight hash-chained
events:

```text
trace_id: trace-live-running2-20261103
Session: term-1793635550991-530889-15
States: running -> artifact_ready -> verified
```

Failure-detection pilot:

```text
trace_id: trace-pilot-missing-20261103
Execution: completed, exit 0
Claimed verification: passed
Actual verification: rejected
Reason: artifact is missing or is not a file: missing.txt
Audit: 6 events, ok
```

This is the useful quality distinction from a raw command pipeline: exit 0 alone is not
accepted as delivery.

## Known limits

- The direct Session execution path maps `session_id`; `pane_id` is optional until
  Workshop exposes it in the synchronous agent-run result. Job execution remains
  observable but is not yet a Work Item dispatch mode.
- Main conversation currently reports the Master result and keeps raw plan/state
  details in `/details` or the Master commands. A dedicated Master transcript card
  is not yet added to the existing TUI.
- Pi, Hermes, Claude and Codex capability measurements remain blocked by local
  auth/model/network conditions and must not be treated as measured reliability.
- Human-baseline comparison requires real pilot tasks; no baseline numbers are
  fabricated here.
