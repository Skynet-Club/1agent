# Product Boundary

Date: 2026-11-03

Status: A0 frozen; ADR-003 accepted; execution remains default-off under rollout

## Product role

1-Agent is a provider-neutral decision and execution runtime. It owns canonical task, plan,
coordination, model, tool, journal, evidence, answer, verification and evaluation contracts.
Model providers and execution hosts are adapters.

The V0 product is optimized for evidence-backed decisions where actions can remain read-only by
default and where unsupported certainty is treated as a failure.

## Supported outcomes

- concise result-first answers with explicit status;
- evidence-backed comparisons and conditional recommendations;
- explicit facts, assumptions, constraints, unknowns, conflicts and flip conditions;
- bounded read-only file evidence and workspace search;
- capability-aware provider routing and deterministic fallback;
- auditable long-running operations with checkpoint and resume;
- gated experience candidates with held-out verification and lifecycle rollback;
- versioned evaluation reports with integrity, content and optional HMAC authentication;
- local host integration through the stdio protocol.
- bounded Master-Agent Work Item DAG planning, approval, Agent selection and supervision;
- controlled Skynet Workshop Session execution with independent Artifact acceptance;
- hash-chained Master Journal replay and false-success rejection.

## Explicit non-goals for V0

- claiming that the deterministic provider is a reasoning model;
- claiming production model quality without configured-provider held-out results;
- autonomous writes or process execution outside the ADR-003 permission model;
- unrestricted or unbounded multi-agent orchestration beyond eight Work Items and three runtimes;
- training or fine-tuning a foundation model;
- changing Skynet Workshop execution authority or duplicating its scheduler;
- storing provider secrets in journals or logs;
- treating hidden model reasoning text as audit evidence;
- treating an evaluation score as external truth without case semantics and reviewer quality;
- treating a content hash or symmetric HMAC as a public-key identity system.

## Permission boundary

- Read roots must be explicit.
- Canonical paths must remain inside configured roots.
- Network access is opt-in.
- Execute-mode requests stop before tool execution unless AuthorityProfile/Policy/Approval and the Workshop sandbox allow them; execution remains default-off under ADR-003 rollout.
- Provider credentials are referenced through environment variables and never written into
  configuration or reports.
- Tool observations are untrusted data with control-character and volume limits.

Additional authority must be introduced as a separate capability with tests for approval,
revocation, replay and audit.

Master execution is an explicit bounded mode:

- the model may propose but cannot self-approve a plan;
- a new plan initializes unapproved;
- path, risk, budget and permission limits are deterministically revalidated;
- each external dispatch requires a durable lease and idempotency key;
- error-exit or missing Artifact cannot become `verified`;
- raw Agent output is not sent wholesale to the planning model.

## Evidence boundary

The following claims are distinct:

1. a claim is user-asserted;
2. a claim is model prior;
3. a claim has an observation;
4. an observation supports a claim;
5. the answer is structurally decision-complete;
6. the evaluation suite passed;
7. the report content is internally consistent and bound;
8. a report was authenticated by a configured key;
9. the decision is externally correct.

The runtime must never collapse these states into one confidence number or one “success” flag.

## Evolution boundary

Experience evolution is L2 and gated:

- candidate generation does not change behavior;
- review and approval are required;
- held-out evaluation must demonstrate benefit without increased unsupported certainty;
- source evidence is digest-bound;
- activation is atomic and scope-unique;
- supersession, degradation, rollback and expiry remain auditable.

No self-modifying code, automatic prompt rewrite, model training or automatic policy activation
is part of V0.

## Platform support matrix

| Platform | Build | Runtime | Filesystem | Cancellation | Status |
|---|---|---|---|---|---|
| Linux x86_64 | verified | verified | verified | tested | supported for V0 validation |
| Windows x86_64 | cross-built | not verified on Windows | not verified | not verified | untested |
| macOS | not verified | not verified | not verified | not verified | untested |

Binary portability alone is not a support claim.

## Provider boundary

DeepSeek, Kimi, Qwen, Ollama and compatible gateways may use the OpenAI-compatible adapter only
when their live behavior satisfies the canonical contract. Adapter compatibility does not imply:

- equal reasoning quality;
- identical tool calling behavior;
- data-residency equivalence;
- stable pricing or latency;
- compatibility with provider-specific reasoning fields.

Provider quality claims require recorded provider/model identity, parameters, held-out results,
latency and cost.

## Allowed and forbidden claims

Allowed:

- “The runtime passes its offline contract and integration suites.”
- “The report is internally consistent and content-bound.”
- “The HMAC approval verified against the supplied registry state.”
- “A configured provider completed a live probe.”

Forbidden without new evidence:

- “The Agent is generally reliable.”
- “Provider X is better for every task.”
- “The system has verified production model quality.”
- “The report is publicly signed.”
- “Windows is supported” before real-host execution.
- “Self-evolution is safe” without repeated held-out and reviewer evidence.

## Expansion gate

Read-only V0 may be integrated behind the frozen terminal as a protocol participant. Further
autonomy, write authority, network tools, asymmetric identity or embedded terminal changes
require new ADRs and the evidence gates in
[`next-phase-evidence.md`](next-phase-evidence.md).
