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

## Versioning rules

- Unknown future answer, journal, policy or summary versions fail closed.
- Known legacy formats may decode through explicit compatibility paths.
- New fields require defaults only when absence has a documented meaning.
- Changing semantics requires a new schema version.
- Content preservation does not imply semantic compatibility.
- Text output may be optimized for humans, but machine contracts require JSON fields and stable
  status values.
