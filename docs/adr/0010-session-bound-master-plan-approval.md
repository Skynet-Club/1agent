# ADR 0010: Session-Bound Master Plan Approval

Status: accepted

Date: 2026-11-03

## Context

The TUI needs `/mission`, `/triage`, `/plan` and `/approve` before Agent assignment and
dispatch are implemented. MasterState initialization currently requires a validated
assignment, which belongs to the next development stage.

Changing `PlanProposal.approved` in the plan file would weaken the existing invariant that
a model-generated plan cannot self-approve and would allow a modified file to masquerade as
an approved plan.

## Decision

Keep the active mission and plan binding in TuiSession v2:

- mission ID, goal, canonical workspace and revision;
- plan path, plan ID, goal ID and SHA-256;
- approval actor and timestamp.

`/plan` uses the existing ModelPlanGenerator, writes the validated PlanProposal under the
session directory and records its SHA-256. The plan file remains `approved=false`.

`/approve` reads the exact file bytes, recomputes the digest and records approval only if
the digest matches the session binding. Session reload also validates plan containment and
digest.

Phase 3 must materialize MasterState from this explicit session approval before creating
leases or dispatching work. The session approval does not itself grant execution authority.

## Consequences

- Model output cannot self-approve a plan.
- A changed plan requires a new binding and a new user approval.
- An approved mission cannot be revised in place; it must be closed or replaced.
- The transition from planning to durable execution state is explicit and auditable.
- Existing MasterState and lease invariants remain unchanged.

## Verification

- mission start, revise, close and plan binding are persistent across reload;
- plan paths outside the session directory fail closed;
- digest mismatch fails approval and session load;
- validated plan generation, stage transition and approval pass a provider-injected test;
- persisted PlanProposal remains `approved=false` after session approval;
- full tests, clippy and rustdoc pass.
