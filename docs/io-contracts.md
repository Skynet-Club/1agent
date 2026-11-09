# Input and Output Contracts

Date: 2026-11-02

Status: V0 contracts implemented; live-provider semantics not yet verified

## Contract layers

```text
user objective
  -> TaskRequest
  -> ModelRequest (provider-neutral)
  -> provider adapter
  -> ModelResponse / ToolCall
  -> ToolObservation
  -> AgentAnswer
  -> RunResult + JournalEvent stream
  -> EvalReport / EvalReportSummary
```

The internal contracts are independent of provider wire formats and host UI.

## TaskRequest

Required:

- `objective`: the requested result or action.

Optional semantic inputs:

- `deliverable`;
- `known_facts`;
- `constraints`;
- `unknowns`;
- `decision_criteria`;
- `success_criteria`;
- `evidence_max_age_ms`;
- `experience_scope`;
- `permissions`;
- `budget`.

The runtime must distinguish missing input, unknown input, inapplicable input and prohibited
access. Defaults are allowed, but must be represented explicitly rather than claimed as user
instructions.

## TUI host and session events

The TUI derives a host binding from explicit Workshop environment fields, with a compatibility
path for the current `SKYTERM_EXPERT_INTERACTIVE`, `SKYTERM_AGENT_ID`,
`SKYTERM_AGENT_RUNTIME` and `SKYTERM_SESSION_ID` contract.

Host bindings are:

- `standalone`;
- `workshop-master`;
- `workshop-worker`.

Conflicting or incomplete signals fail closed to `standalone`. Command visibility uses this
binding plus the current lifecycle stage, but execution handlers must independently validate
Master state and Workshop authority.

Session metadata is version 2 and remains backward compatible with version 1. Each session has
an append-only `events.jsonl`; records contain a schema version, contiguous sequence, timestamp
and one of:

- session opened;
- command accepted;
- command rejected;
- task submitted;
- mission started, revised or closed;
- plan bound or approved;
- session closed.

Session events do not replace the run journal. Model-visible messages, tool calls and observations
remain reconstructable from the run journal.

Version 2 session metadata may contain one active mission. A mission records its stable ID,
goal, canonical workspace, revision and an optional plan binding. The plan binding contains the
absolute plan path, plan ID, goal ID, content SHA-256, approver and approval timestamp.

The plan file itself remains `PlanProposal.approved=false`. Approval is separate and fails if the
file digest differs from the bound digest. Loading a session fails closed when a bound plan
escapes the session directory, is missing, exceeds 4 MiB or no longer matches its digest.

An approved session plan may bind one `AssignmentProposal`. Assignment must target the same
plan ID. Dispatch requires both approval and assignment, then stores the durable Master trace
ID and contained state-root reference in the session plan binding.

The dispatch bridge initializes or reloads the existing MasterState, materializes the recorded
approval in the Master journal, creates leases and delegates Session creation/write operations
to the Workshop adapter. Session approval alone never grants execution authority.

Supervision uses the same durable state:

- `/watch` maps bounded progress markers to `Running`, `AwaitingInput` or `Blocked`, and maps a
  terminal exit receipt to `ArtifactReady`, `Rejected`, `Blocked` or `Cancelled`;
- `/respond` is accepted only for `AwaitingInput` or `Blocked`, writes the operator message to the
  bound Workshop Session with `session write --require-idle`, then records `Running`;
- control decisions append `Paused`, `Resumed`, `TakenOver`, `Cancelled`, `RetryRequested` or
  `ReplanRequested` events after the corresponding state transition;
- cancel must be legal for the current task state before the Session is signalled; retry and
  replan state/limit checks likewise precede Session cancellation;
- `/attention` includes `AwaitingInput`, `ArtifactReady`, `Blocked`, `Rejected`, `Retrying`,
  `Replanning` and expired active leases;
- `/timeline` combines bounded Master and Workshop tails. Workshop events are correlated only
  when their bounded payload contains a Session ID present in MasterState.

Verification has one explicit input path:

```text
/verify <task-id> <verification.json>
```

The JSON carries criterion results, optional test results, independent evidence and a verifier
identity. The TUI computes Artifact checks from the current Work Item workspace; callers do not
provide authoritative Artifact state. Verification rejects:

- a verifier identity equal to the assigned execution Agent;
- an execution receipt that did not complete with exit code zero;
- missing or duplicate criterion results;
- missing, non-regular or workspace-escaping Artifacts;
- pinned SHA-256 drift;
- failed tests or no independent evidence.

`/artifacts` is a read-only filesystem view. `/verify` is the only phase-five command that mutates
Master state. Master `/evidence`, `/audit` and `/report` are deterministic views over Master
journal replay and the current Artifact filesystem.

## Session preferences and context compaction

Session version 2 retains version 1 compatibility and adds optional local metadata:

- bounded step, tool-call, provider-attempt, context-character and timeout preferences;
- an auto-compaction threshold from 50% to 95%, defaulting to 80%;
- preferred canonical workspace;
- canonical mentioned paths;
- archived state;
- a deterministic context summary with covered-journal count and creation timestamp.

Runtime context compaction clears old non-recent tool-result payloads when the
context-character budget is exceeded. `/context compact` instead summarizes completed run
objectives and final conclusions locally. The summary is injected only while the
covered-journal count matches the current session journal count; a new run invalidates it
automatically.

Character counts are not provider token counts. `/context show` and `/status` therefore label
the remaining room as character headroom.

Every provider response records `Usage { input_tokens, output_tokens, cost_micros }`.
`/cost` derives a read-only ledger from all `ModelResponded` events. A provider-reported
non-zero cost is preserved. When cost is zero, the runtime computes configured pricing from
the selected route manifest at micro-units per 1,000 tokens. The ledger distinguishes
zero-cost calls, reports provider/model and run groups, and does not claim a currency or
include Agent processes outside 1A journals.

`/session delete <id> confirm <id>` is recoverable: it renames the session directory to
`.trash`. It rejects deleting the active session and never deletes by prefix or glob.

Preset files carry a schema version, bounded safe name and instruction. Exports use create-new
file semantics. Workspace mentions must canonicalize below configured read roots.

TUI first-principles command runs use an explicit `CognitiveMode` override that is
written to `RunStarted`. Resume continues using the checkpoint mode and does not accept a new
override. Commands that only inspect evidence, audit a journal or render a report do not invoke a
provider and derive their output from journal replay.

## Permissions

```text
read_roots
allow_network
allow_write
```

Default:

- no write authority;
- no network authority;
- reads only below explicitly configured roots.

Execute-mode requests without authorization stop before provider or tool execution.

## AgentAnswer

Current schema version: `2`. Legacy schema `1` remains readable for known historical events.
Unknown future versions fail closed.

Required:

- `schema_version`;
- `status`;
- `conclusion`.

Typed semantic fields:

- `claims`;
- `evidence`;
- `assumptions`;
- `constraints`;
- `unknowns`;
- `conflicts`;
- `flip_conditions`;
- `action_receipts`.

Answer status:

```text
completed | conditional | needs_evidence | needs_authorization | failed
```

Core invariants:

- verified claims reference declared sources, with matching evidence;
- evidence references declared claims and does not duplicate claim IDs;
- high-impact unknowns have a reduction method;
- high-impact assumptions have a verification method;
- a conditional answer has a target-bound flip condition;
- conflicting claims have an unresolved conflict record or explicit resolution;
- read-only requests cannot contain action receipts;
- expired or untimed `verified` evidence fails freshness policy when required.

## Model contracts

`ModelRequest` contains:

- run ID;
- cognitive mode;
- canonical messages;
- offered tool specifications.

`ModelResponse` contains:

- provider ID and model ID matching the selected route;
- either a final `AgentAnswer` or tool calls;
- usage counters.

Provider-specific fields remain in adapters. Normalized provider errors contain a stable code,
sanitized message and retryable flag. Credentials and full request headers are never journaled.

## Tool contracts

Each tool call has a unique ID, registered name and schema-validated JSON arguments.

Each observation has:

- matching call ID and tool name;
- status;
- bounded summary/content;
- optional stable error code;
- metadata without secrets.

Every call must have exactly one matching observation in a completed tool step. Retrieved content
is untrusted data and is checked for disallowed invisible control characters.

## Journal

The append-only JSONL journal is the run fact source. Each event includes:

```text
schema_version
run_id
seq
timestamp_unix_ms
event
```

The audit checks contiguous sequence, run identity, terminal state, model/tool pairing, route
consistency, retry timing and answer provenance. A model-visible request must be reconstructable
from journal events.

Journal replay must expose the same terminal stop reason and tool pairing without calling a
provider.

## Checkpoint and resume

Checkpoints contain:

- cognitive mode and canonical messages;
- consumed tool and provider-attempt budgets;
- unavailable routes;
- next step and elapsed time;
- route-plan digest and policy sources;
- tool-capability digest.

Resume fails closed when routing policy or checkpoint-bound capabilities change.

## Evaluation reports

`EvalReport` contains:

- suite and kind;
- total/pass/fail and hard-gate counts;
- rubric and decision-completeness aggregates;
- minimum completeness floor and source policy;
- canonical `content_sha256`;
- per-case results.

Validation recomputes all aggregate fields from case results. A retained digest mismatch fails
inspection.

`EvalReportSummary`:

- has an independent schema version;
- excludes case content;
- carries policy provenance and report digest;
- reports integrity, content binding, signature presence and authenticity separately.

Reports without policy/digest metadata remain readable as legacy documents. Unsupported policy or
summary versions fail closed.

## Authentication

`eval-inspect` can verify an `ApprovalEnvelope` with artifact kind `evaluation_report`.

Authentication requires:

- exact report-byte digest match;
- valid HMAC-SHA256 tag;
- Active or Retired key in the supplied registry;
- non-revoked key;
- key length of at least 16 bytes.

This is symmetric shared-key authentication. It is not a public-key signature or external identity
proof.

## Master-Agent contracts

Master planning and coordination use separate versioned contracts:

- `TriageDecision`: route, reason, Work Item ceiling, planning and approval requirements.
- `PlanProposal`: at most eight Work Items with dependencies, budgets, permissions, risk,
  acceptance criteria and expected Artifacts.
- `AssignmentProposal`: exactly one task-to-Agent assignment per Work Item, with a
  deterministic selection reason.
- `TaskLease`: task, Agent, execution target, generation, expiry and idempotency key.
- `ExecutionReceipt`: lease generation, idempotency key, Session/Pane/Job target, execution
  status, exit code, Artifact references and normalized error.
- `VerificationInput`: exact criterion results, test results, independent evidence, Artifact
  checks and verifier identity.
- `VerificationResult`: verified/rejected status, checks, evidence and failure reason.
- `MasterJournalEvent`: contiguous sequence, trace/plan identity, event kind and complete
  post-transition `MasterState`.
- `CapabilityProbeManifest`: bounded suite/runtime commands, prompt delivery, timeout and
  explicit success/blocked rules.
- `CapabilityProbeReport`: suite and manifest digests, normalized result status, exit code,
  duration and output byte/digest metadata without raw runtime output.

### Production-line Goal and key results

`ProductionPlan` wraps the existing `PlanProposal` with:

- `GoalContract`: safe Goal and Master-conversation identity, objective, non-goals and
  constraints.
- `KeyResult`: stable ID, measurable statement, baseline, target, measurement method and
  assigned Work Item IDs.

The wrapped plan remains unapproved. A production-line plan that claims `approved=true`
fails closed because approval is a separate session-bound user action.

A Goal with more than one Work Item must contain at least one measurable Key Result. When Key
Results are present, every Work Item must belong to exactly one Key Result. A single direct
Work Item may omit Key Results to avoid inventing OKR structure.

Plan digest input excludes runtime state. Approved plan and task-list bytes remain immutable;
execution status belongs to `MasterState`.

`TaskListSnapshot` is produced by a deterministic renderer from the validated structured
plan. It records:

- current task-list path;
- plan SHA-256;
- task-list Markdown SHA-256.

The file is stored under `<state-root>/<goal-id>/v<plan-version>/任务清单.md`. Reusing the same
revision with different content fails closed. Invalid plans create no task-list path.

In Workshop Master mode, a plain actionable sentence starts the same production flow:
deterministic triage creates a Mission, the provider proposes a `ProductionPlan`, and the
TUI displays a non-executing preview. `确认计划` / `/approve` persists the task-list
snapshot. `确认执行` / `/dispatch run` requires the bound task-list and plan digests to
still match. Questions such as `如何优化…` remain normal conversation rather than creating
a Goal.

`SessionPlan` stores the task-list path, Markdown digest and canonical production-plan
digest. Dispatch recomputes all three before creating a lease. A deleted or modified task
list fails closed.

Global multi-conversation ordering uses `ConversationQueue`, `ScheduledTask`,
`SchedulerLimits` and `SchedulerDecision`. Task identity in this contract is
`<conversation_id>:<task_id>`. The scheduler:

- counts existing active tasks against global and per-conversation limits;
- starts only tasks whose dependencies are `verified`;
- selects eligible conversations round-robin;
- serializes write tasks whose workspace paths overlap;
- reports an explicit waiting reason for every nonterminal task.

The CLI entry is:

```bash
1agent master schedule --input queues.json \
  --global-active 6 --per-conversation 3 --json
```

Plan objects reject unknown fields and model-proposed plans cannot be approved. Workshop
response adapters intentionally ignore unknown response fields so additive platform changes
remain compatible; all fields consumed by routing or verification are still mandatory.

An expected Artifact is a relative path with no `..`; optional
`<path>@sha256:<64-hex-digest>` pins its content. A verification with missing criteria,
missing evidence, a non-regular file, path escape, digest mismatch, test failure or non-zero
execution is rejected.

Master Journal load uses replay rather than trusting the snapshot cache. Audit rejects
non-contiguous sequence, identity drift, unsupported schema, decreasing bounded counters and
illegal task status transitions.

## Versioning rules

- Unknown future answer, journal, policy or summary versions fail closed.
- Known legacy formats may decode through explicit compatibility paths.
- New fields require defaults only when absence has a documented meaning.
- Changing semantics requires a new schema version.
- Content preservation does not imply semantic compatibility.
- Text output may be optimized for humans, but machine contracts require JSON fields and stable
  status values.
