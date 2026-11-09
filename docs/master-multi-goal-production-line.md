# 1A + Skynet Workshop 多目标生产线开发计划

Date: 2026-11-04

Status: proposed

Owners:

- 1A: goal, key result, task definition, planning, approval, supervision and verification.
- Skynet Workshop: Master conversations, worker panes, input policy, global capacity and
  execution events.

Indicative targets:

- 1A `v0.95.x`
- Skynet Workshop `v0.12.x`

## 1. Product definition

The Master-Agent is a production-line controller, not a chat persona. A Workshop user can
create multiple Master conversations. Each conversation owns one active Goal. A Goal is
expressed as one or more measurable Key Results, and each Key Result is delivered by one
or more stable Work Items.

```text
Skynet Workshop
  Master conversation
    Goal
      Key Result
        Work Item
          Execution Session
            Pane
```

Multiple Goals may run concurrently in separate Master conversations. They share one
Workshop process, Agent Registry, event stream and global capacity budget.

Example:

```text
Master 1 / Goal 1
  KR 1.1 -> T1, T2
  KR 1.2 -> T3, T4

Master 2 / Goal 2
  KR 2.1 -> T5
  KR 2.2 -> T6, T7
  KR 2.3 -> T8, T9
```

The counts are illustrative. Plans derive tasks from deliverables and dependencies rather
than reverse-engineering a requested task count.

## 2. Product decisions

1. One Workshop window hosts multiple Master conversations.
2. Each Master conversation has at most one active Goal.
3. Several Master conversations and Goals may run concurrently.
4. A Goal can contain multiple measurable Key Results.
5. Multi-task Goals require at least one measurable Key Result.
6. A simple read-only request may remain a single direct analysis task without fabricated
   Key Results.
7. Task identity is stable; Pane addresses are ephemeral.
8. The approved plan revision is immutable.
9. Natural-language task intent produces a reviewable proposal, never immediate writes.
10. Plan approval and dispatch approval are separate user decisions for multi-task or
    side-effecting work.
11. The approved task list is a deterministic Markdown snapshot owned by 1A state.
12. Worker panes are observable but input-locked while under Master supervision.
13. Users can always communicate through the Master; direct worker input requires an
    explicit takeover.
14. A retry creates a new execution Session while preserving the stable Work Item ID.
15. Execution success is not acceptance. Independent verification remains mandatory.

## 3. Non-goals

- An enterprise OKR or performance-management system.
- Unbounded multi-Master or multi-Agent creation.
- A second Work Item scheduler inside 1A.
- Letting the planning model approve its own plan or expand permissions.
- Direct free-form input into supervised Worker panes by default.
- Treating raw Worker logs as trusted instructions.
- Automatically creating a new Goal when the user asks a side question.
- Allowing tasks in different Master conversations to mutate the same write surface
  concurrently without coordination.

## 4. Terminology and identity

### MasterConversation

A durable conversation with its own 1A TUI session, active Goal, history and Master
journal. `Master 1`, `Master 2` are display names; they are not separate daemon identities.

### Goal

An outcome the user explicitly accepted:

```text
goal_id
master_conversation_id
objective
non_goals[]
workspace
constraints[]
permissions
budget
key_results[]
state
plan_revision
```

### KeyResult

A measurable result, not a task:

```text
kr_id
goal_id
statement
baseline
target
measurement_method
acceptance_evidence[]
work_item_ids[]
```

### WorkItem

A stable, independently assignable and verifiable unit:

```text
task_id                 # globally unique, e.g. M1:T3
goal_id
kr_id
title
objective
depends_on[]
required_capabilities[]
required_tools[]
permissions
acceptance_criteria[]
expected_artifacts[]
budget
risk
state
```

`1-3#` is a current human-readable Pane address. Dragging panes, changing tabs or
recovering a Session may change that address. Commands, leases, evidence and audit records
must use `task_id`, never a Pane address as identity.

### ExecutionBinding

```text
task_id
attempt
agent_id
session_id
pane_id
lease_generation
state
```

One Work Item may have multiple historical execution bindings. Only one binding can be
active at a time.

### PlanRevision

An immutable Goal plan accepted by the user:

```text
revision
plan_sha256
task_list_sha256
created_at
approved_at
approved_by
supersedes
```

Changes never mutate an approved revision. They create a new revision and display the
impact before approval.

## 5. User interaction

### 5.1 Creating a Master conversation

The Workshop conversation strip adds:

```text
[+] New conversation
```

The new item is shown as `Master 2`, then renamed to a Goal-based title after plan
creation. It is a new Master session inside the same Workshop window, not a second
top-level window and not a second daemon.

Creating a conversation does not start an Agent, create worker panes or reserve budget.

### 5.2 Natural-language task intake

Plain text is classified within the current conversation and Goal state:

```text
Question
NewTaskIntent
Clarification
OperatorResponse
TaskAmendment
ChangeRequest
ControlCommand
NewGoal
```

Rules:

- A question such as “这个项目有什么问题” is answered without creating a Goal.
- An actionable request produces a non-executing Goal/plan preview.
- If intent or target is ambiguous, ask at most the minimum blocking questions.
- While a Goal is running, an unrelated actionable request asks whether to create another
  Master conversation; it never overwrites the active Goal.
- New task intent in a completed conversation starts a fresh Goal in that conversation.

### 5.3 First confirmation: approve the plan

The plan preview includes:

- Goal objective and non-goals;
- measurable Key Results;
- Work Items, dependencies and parallel groups;
- proposed Agents and capabilities;
- read roots, write roots, network and upload permissions;
- expected artifacts and acceptance criteria;
- per-task and global budgets;
- risks and unresolved assumptions.

The user action is named `确认目标与拆分`, not `确认执行`.

Before this approval, no target-project write, worker Session or dispatch lease is created.

### 5.4 Task list snapshot

After plan approval, 1A deterministically renders and atomically stores:

```text
<master-state-root>/<goal_id>/<plan_revision>/任务清单.md
```

The file is a render of the structured plan, not a second model-authored plan. It includes
the plan ID, revision, SHA-256, Goal, Key Results, Work Items, dependencies, permissions,
agents and acceptance criteria.

The Markdown snapshot is immutable. Runtime status remains in `MasterState`; the Workshop
board is the live source. Manual edits do not silently change execution. If the snapshot
does not match its recorded digest, dispatch fails closed and requires a new revision.

### 5.5 Second confirmation: authorize dispatch

The `Prepared` screen shows:

```text
Plan: 任务清单.md
SHA-256: ...

Goal 1
  T1 -> Codex
  T2 -> Codex
  T3 -> Pi
  T4 -> release-verifier

Ready now: T1
Queued by dependencies: T2, T3, T4
Write roots: ...
Global concurrency request: ...

[确认执行并派工] [修改计划] [取消]
```

Only the second action creates execution leases, Sessions and worker panes.

Low-risk, single-task, read-only work may later offer a compact one-step approval. That is
an optimization after the two-stage contract is stable, not part of the first milestone.

### 5.6 Confirmation state

```text
Draft
  -> Planned
  -> Approved
  -> Prepared
  -> Dispatching
  -> Supervising
  -> Verifying
  -> Accepted
```

Exceptional states:

```text
Blocked
AwaitingApproval
Cancelled
Failed
```

The Goal state and each Work Item state are independent. A Goal is not complete until all
required Work Items are verified and its Key Result acceptance is satisfied.

## 6. Supervision interaction

The Master input remains available during `Supervising` and `Verifying`. It is not a
view-only mode.

### 6.1 Input routing

| Input | Action |
|---|---|
| Status question | Read the board and answer |
| Answer to `AwaitingInput` | Route to the exact task Session |
| Clarification / narrowing constraint | Send to affected running tasks and require ACK |
| Scope, dependency, budget, permission or acceptance change | Pause affected work and propose a new plan revision |
| Pause/resume/cancel/retry/takeover | Apply the validated control transition and journal it |
| Unrelated actionable request | Offer a new Master conversation or queue a new Goal |

Every operator message receives a stable message ID. Delivery and acknowledgement are
separate states.

### 6.2 Worker pane input policy

Worker panes under supervision default to `Observe`:

- viewing, scrolling, selecting and copying are allowed;
- keyboard input, paste and middle-click paste are blocked;
- layout, zoom and detailed event inspection remain allowed;
- the pane displays `只读 · 通过 Master 发言`.

The user can explicitly choose `接管任务`. That action:

1. transitions the task to `HumanTakeover`;
2. pauses automatic retry and replan behavior;
3. preserves the current Session and lease;
4. unlocks direct input for that pane;
5. journals operator input and the reason;
6. requires reconciliation before returning to Master supervision.

A `Responding` state exposes a dedicated reply field for an Agent question without
unlocking the entire terminal.

### 6.3 Pause semantics

The UI and protocol distinguish:

- `pause dispatch`: stop starting new Work Items;
- `pause task`: stop routing new instructions to a task while it reaches a stable state;
- `cancel task`: terminate the active execution and cancel the Work Item;
- `emergency stop`: cancel all active executions in the Goal and stop dispatch.

No single `pause` label may silently mean all four.

### 6.4 Retry and replan

- Retry keeps `task_id` and creates a new attempt, Session and execution binding.
- Automatic retry remains capped.
- Automatic replan remains capped and restricted to the failed dependency subgraph.
- A user-requested scope change always creates a new plan revision.
- Completed and verified tasks are not re-executed unless explicitly included in the
  revision.

## 7. Verification

Each Work Item declares a Definition of Done:

- expected artifacts;
- path containment;
- optional SHA-256;
- exact acceptance criteria;
- deterministic tests or observable evidence;
- verifier identity distinct from the executing Agent.

An execution exit code of zero makes the task `ArtifactReady`, not `Verified`.

Key Results are complete only when their declared measurement and evidence are present.
The final Goal report separates:

```text
planned
executed
artifact_ready
verified
accepted
blocked
cancelled
```

## 8. Architecture and ownership

```text
1A
  intent router
  Goal / KR / Work Item domain
  plan revisions and approval
  task list rendering
  Master journal and supervision
  independent acceptance

Skynet Workshop
  Master conversation UI
  worker pane allocation
  global capacity leases
  Session / Job / PTY lifecycle
  input policy and takeover gate
  event stream and operator surfaces
```

Workshop remains the only owner of PTYs, Sessions, Jobs, Pane allocation and process
capacity. 1A remains the only owner of Goal/plan approval and Work Item supervision.

The global scheduler is deterministic and non-LLM. It arbitrates:

- total active Work Items;
- per-Master concurrency;
- per-provider/runtime concurrency;
- workspace write reservations;
- CPU, memory, network and cost budgets;
- queued versus runnable dependencies.

Read-only tasks on the same workspace may run concurrently. Write-bearing tasks targeting
the same workspace or overlapping paths default to serial execution unless isolated
worktrees are explicitly approved.

## 9. Protocol changes

### 9.1 Host binding

The Workshop must set the explicit v1 contract:

```text
SKYTERM_HOST_CONTEXT=skynet-workshop
SKYTERM_HOST_CONTEXT_VERSION=1
SKYTERM_AGENT_ROLE=master
SKYTERM_MASTER_CONVERSATION_ID=master-...
SKYTERM_AGENT_RUNTIME=1agent
SKYTERM_SESSION_ID=term-...
```

The legacy requirement that Master identity must equal `main-coordinator` needs a
compatibility path. Conversation 1 may continue to use `main-coordinator`; later
conversations must have unique role-bound identities rather than duplicate primary Agent
records.

### 9.2 Events

Versioned events required by the operator UI:

```text
goal.proposed
goal.approved
plan.revised
plan.approved
task_list.created
dispatch.approved
task.queued
task.started
task.awaiting_input
task.blocked
task.artifact_ready
task.verified
task.rejected
lease.expired
operator.message
operator.ack
human_takeover.started
human_takeover.ended
goal.accepted
goal.cancelled
```

Events are bounded, typed and correlated by `goal_id`, `task_id`, `session_id` and
`message_id`. The UI must not infer lifecycle state by scraping full terminal output.

## 10. Implementation phases

### M0: Contract and compatibility audit

Tasks:

- audit every assumption that there is exactly one `main-coordinator`;
- define `MasterConversation`, Goal, KR and global scheduler contracts;
- define event and error schemas;
- identify old settings/state that need read compatibility;
- add ADRs for multi-conversation identity, plan revisions and worker input policy.

Verification:

- two isolated 1A TUI sessions can coexist without sharing Mission state;
- the existing single-Master path still passes;
- no destructive migration is introduced.

Exit gate:

- approved ADRs and no unresolved state-ownership conflict.

### M1: Goal, Key Result and task domain

Tasks:

- add Goal/KR models and validation;
- map Work Items to one KR within the same Goal;
- assign globally unique stable task IDs;
- add budget and acceptance validation;
- implement a deterministic task-list Markdown renderer with digest checks.

Verification:

- 2 KR / 4 task and 3 KR / 5 task plans validate;
- blank, duplicate or non-measurable KRs fail closed;
- task rendering is byte-stable for the same plan;
- Task ID survives Pane movement and Session recreation.

Exit gate:

- domain tests and contract fixtures pass without GUI.

### M2: Natural-language intake and two-stage approval

Tasks:

- add Master intent routing;
- generate non-executing Goal and plan previews for task intent;
- implement `确认目标与拆分` and `确认执行并派工`;
- persist plan approval and task-list snapshot atomically;
- support revise/cancel without creating an execution Session.

Verification:

- a question creates no Goal;
- a task intent creates no target-project write before plan approval;
- plan approval creates a task list but no Session;
- dispatch approval creates only runnable tasks;
- a modified plan invalidates the previous approval.

Exit gate:

- complete single-Master PTY workflow test.

### M3: Dispatch, panes and Goal workspace layout

Tasks:

- allocate one worker slot per Work Item;
- map stable Task IDs to current Session and Pane IDs;
- create one Goal-oriented workspace page;
- start Sessions only for ready tasks;
- queue dependency-blocked and capacity-blocked tasks visibly;
- expand one Goal across tabs using Task IDs rather than address assumptions.

Verification:

- T1..T4 appear as planned task slots and only ready tasks start;
- moving or renaming panes does not change task ownership;
- exceeding the visible pane capacity queues work instead of losing tasks;
- cancelled tasks release their slot and lease.

Exit gate:

- full dispatch and board smoke test with no orphan Sessions.

### M4: Bidirectional supervision and input policy

Tasks:

- enable Master Q&A while tasks run;
- route operator answers and amendments to exact tasks;
- add message delivery and ACK tracking;
- block direct input in supervised worker panes;
- add `Responding`, `HumanTakeover`, pause and emergency-stop flows;
- generate plan revisions for scope-changing input.

Verification:

- status questions never alter task state;
- an awaiting-input answer reaches exactly one task;
- direct typing and paste are blocked in `Observe`;
- takeover unlocks one pane and pauses automatic decisions;
- scope expansion pauses affected work and requires a new approval;
- every delivered message has an ACK or an explicit delivery failure.

Exit gate:

- PTY acceptance covers question, response, amendment, change request, takeover and
  emergency stop.

### M5: Multiple Master conversations and global scheduling

Tasks:

- add `+ New conversation`;
- create isolated Master sessions and Goal roots;
- add a deterministic production overview;
- implement global concurrency, provider and workspace reservations;
- prevent conflicting write tasks from starting concurrently;
- support two Goals from separate conversations with independent approvals.

Verification:

- Goal 1 with 2 KRs / 4 tasks and Goal 2 with 3 KRs / 5 tasks can be prepared together;
- each Goal has a distinct plan and task-list digest;
- cancelling one Goal does not mutate the other;
- global limits hold under concurrent dispatch;
- overlapping write roots serialize unless worktree isolation is approved;
- the overview reports every queued, running and awaiting task exactly once.

Exit gate:

- multi-Goal end-to-end test with at least nine logical Work Items and a global cap below
  nine.

### M6: Recovery, verification and release hardening

Tasks:

- recover `Prepared`, `Dispatching`, `Supervising` and `Verifying` states after restart;
- prevent duplicate dispatch through idempotency keys;
- reconcile interrupted Sessions and expired leases;
- complete independent KR and Goal acceptance;
- add operator reports, timeline and cost summaries;
- run soak, fault-injection and upgrade tests.

Verification:

- crash before task-list write leaves the Goal undispatchable;
- crash after plan approval recovers the same snapshot and approval;
- crash during dispatch never creates a second lease for the same attempt;
- GUI/sessiond restart does not duplicate or silently accept tasks;
- incomplete evidence cannot produce `Accepted`.

Exit gate:

- full release gate and documented rollback.

## 11. Acceptance scenarios

1. **Question only**: “这个项目的核心风险是什么？” returns analysis and creates no Goal.
2. **Plan rejection**: task intent displays a plan; cancel creates no file, Session or
   lease.
3. **Two-stage plan**: first confirmation creates the task-list snapshot; second
   confirmation dispatches.
4. **Two concurrent Goals**: 2 KR / 4 task and 3 KR / 5 task plans run under one global
   cap without identity or budget leakage.
5. **Supervision question**: “现在做到哪了？” answers from the board.
6. **Awaiting input**: Agent asks a question; the user's answer reaches only that task.
7. **Narrow amendment**: a new constraint reaches affected running tasks and receives an
   ACK.
8. **Scope change**: a new deliverable produces Plan v2, pauses affected tasks and
   requires approval.
9. **Pane movement**: moving `T3` from `1-3#` to another tab preserves `M1:T3`.
10. **Input lock**: direct typing and paste fail in `Observe`; viewing and copying work.
11. **Takeover**: explicit takeover unlocks one worker and disables automatic decisions.
12. **Failure and retry**: retry keeps the Task ID and creates a new Session.
13. **Crash recovery**: restart restores decisions without duplicate Sessions.
14. **Verification**: exit 0 without matching criteria remains unaccepted.

## 12. Test strategy

- domain unit tests for Goal/KR/plan/task validation and digest stability;
- schema and migration tests for old Master/TUI state;
- fake-provider tests for intent routing and plan generation;
- PTY tests for intake, approval, supervision, input lock and takeover;
- Workshop GUI tests for conversation creation, task slots and live board;
- scheduler property tests for capacity, dependency and workspace locks;
- crash-injection tests at every durable transition;
- negative tests for direct input, plan mutation, self-verification and duplicate dispatch;
- two-Goal integration test with 4 + 5 Work Items;
- release packaging, install and rollback tests.

## 13. Observability and metrics

Product metrics:

- time from task intent to reviewable plan;
- percentage of tasks dispatchable without user correction;
- awaiting-input response latency;
- task verification pass rate;
- retry and replan rates;
- blocked and cancelled task reasons;
- duplicate dispatch, orphan Session and lease-expiry counts;
- per-Goal and per-provider token/cost usage;
- operator interventions per completed Key Result.

Metrics must be derived from typed events and journals, not terminal text scraping.

## 14. Compatibility and rollout

- Keep existing `/mission`, `/plan`, `/approve`, `/respond`, `/control` commands as
  advanced interfaces.
- Preserve the existing single-Master flow behind a feature flag until the new intake and
  dispatch path passes PTY tests.
- Read old session and Master state without rewriting it.
- Use new state directories and schema versions; no in-place destructive migration.
- Introduce the UI in this order: single Master intake, task list, supervision lock,
  multiple Master conversations.
- Roll back by disabling the feature flags and reopening the existing single-Master view.

## 15. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Confirmation fatigue | Differentiate plan approval and dispatch approval; defer one-step low-risk mode |
| Plan/task-list drift | Deterministic rendering and digest mismatch fails closed |
| Context explosion | Keep domain state external; send compact views to the model |
| Pane identity confusion | Stable Task IDs and dynamic pane bindings |
| Duplicate dispatch | Leases, idempotency keys and crash-injection tests |
| Over-parallelization | Global capacity and workspace reservations |
| Silent scope expansion | New plan revision and explicit approval |
| Worker prompt injection | Typed status markers; Worker output is untrusted |
| Unsafe direct intervention | Observe by default, explicit takeover and journaling |
| Multiple Masters conflict | One active Goal per conversation plus a deterministic global overview |

## 16. Definition of done

The first production release is done when:

- a user can create two Master conversations and define two Goals;
- each Goal can contain multiple KRs and independently approved Work Items;
- natural-language task intent produces a plan review without side effects;
- plan approval creates a deterministic task-list snapshot;
- dispatch approval creates isolated Sessions and visible task panes;
- supervision accepts questions, responses, amendments and control commands;
- direct worker input is locked until explicit takeover;
- global scheduling prevents capacity and write-surface conflicts;
- every completed Work Item has independent evidence;
- crash recovery preserves approvals and never duplicates dispatch;
- all release, migration and rollback gates pass.
