# ADR 0017: Goal, Key Result and Task List Snapshots

- Status: proposed
- Date: 2026-11-04

## Context

Master-Agent V1 has one Mission and a bounded `PlanProposal`. The next production-line
iteration needs explicit Goals, measurable Key Results and multiple Master conversations.
The user also needs a durable task-list artifact before dispatch.

A model-authored Markdown file would create a second plan source and could drift from the
structured plan. Mutable task lists would also make later approval and audit ambiguous.

## Decision

1. Add a production-line contract above the existing `PlanProposal`.
2. A Goal contains zero or more measurable Key Results.
3. A Goal with more than one Work Item must contain at least one Key Result.
4. Every Work Item belongs to exactly one Key Result when Key Results are present.
5. A Key Result must declare a baseline, target and measurement method.
6. Work Item IDs remain the stable execution identity. Pane addresses are not identities.
7. Approved plans and task-list snapshots are immutable.
8. `任务清单.md` is rendered deterministically from the structured plan. It is never a
   separate model-authored plan.
9. The snapshot records the plan digest. Changes create a new plan revision.
10. Existing Mission/Plan commands and stored plans remain readable during migration.

## Rejected alternatives

- Let the planner write Markdown directly: creates two conflicting sources of truth.
- Mutate the Markdown during execution: blurs approved scope and runtime status.
- Make Key Results mandatory for single direct tasks: produces artificial goals.
- Use Pane addresses as task identity: breaks after pane movement, recovery or retry.

## Consequences

- The domain must validate Goal/KR/task coverage before a task list can be generated.
- A deterministic renderer and digest checks become part of the approval path.
- Runtime status remains in MasterState and is projected into the Workshop board rather
  than mutating the approved snapshot.
- A later revision can supersede a plan without rewriting its audit history.
