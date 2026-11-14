# ADR 0011: Session-to-Master Dispatch and Explicit Route Selection

Status: accepted

Date: 2026-11-03

## Context

Mission and plan approval live in the TUI session until assignment is possible. Execution
must not occur directly from that session record, and model selection must not bypass
router hard filters or provide a second scheduler.

## Decision

Add TUI controls for model route inspection/selection, per-run permissions and tool
inspection. Provider selection is applied after normal router filtering: an ineligible
selected route has no fallback path. Session-level network, write and read-root changes
apply only to the next runs in that TUI session.

`/agents` merges configured Agent profiles with bounded Workshop Registry metadata and
caches the resulting profiles under the session directory.

`/assign` uses `PlanProposal::assign` and binds exactly one AssignmentProposal to the
approved session plan.

`/dispatch`:

1. verifies plan digest, approval, assignment and cached profiles;
2. initializes or reloads a durable MasterState;
3. records the session approval in the Master journal;
4. creates leases with generations and idempotency keys;
5. delegates Session creation and writes to Workshop.

The session binding stores the Master trace ID and contained state root. Workshop remains
the only Session/Pane/process scheduler.

## Consequences

- TUI approvals become durable execution state only through an explicit dispatch step.
- Repeated dry-runs and dispatch preparation are idempotent for the same plan trace.
- Agent assignment remains deterministic and explainable.
- A pinned model can fail closed instead of silently using another provider.
- Permission changes are visible, explicit and scoped to the session.

## Verification

- provider pinning and tool inspection are tested;
- permissions control the next TaskRequest and default to read-only;
- configured Agent profiles survive Workshop discovery failure;
- assignment is bound only to an approved plan with the same plan ID;
- dispatch creates an approved MasterState and Running task with an active lease and
  Session target using a fake Workshop adapter;
- full tests and static checks pass.
