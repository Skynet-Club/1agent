# ADR 0089: CUA Single-Shot Plan

- Status: accepted
- Date: 2026-11-12

## Context

The CUA policy already separates argument provenance, quarantines untrusted
screen content from the planner and rejects branch targets outside a planned
set. It did not make the plan lifecycle explicit: nothing required an approved
plan to be frozen before the first action, so a later observation could still
change the branch set.

## Decision

1. Add `CuaPlan` with a validated `plan_id`, a non-empty set of unique branch
   names and a `frozen` flag.
2. `CuaPlan::freeze` is the approval step. `authorize(action, target)` denies
   every action until the plan is frozen; after freezing it checks the branch
   against the frozen set, then applies `evaluate` for provenance/risk.
3. `CuaPlan::replan` replaces branches only before freezing. After freezing it
   returns `deny` with the reason that replanning after execution is branch
   steering, and leaves the frozen branches unchanged.
4. Plan ids and branch names are bounded to 64 safe characters; duplicate or
   empty branches fail closed.

## Consequences

- The safety contract now has a testable single-shot lifecycle: plan → freeze →
  authorize, with no mid-execution replanning.
- This is policy only. There is still no browser executor, screenshot, click or
  typing; a future executor must call `authorize` before every action.
- The planner still must not see untrusted content; `planner_sees_untrusted`
  remains a separate guard.
- A legitimate plan change after execution starts requires a new plan and a new
  approval, not a mutation of the frozen one.

## Evidence

```bash
cargo test cua --lib
cargo test --test cua_check_cli
```

The tests cover an unfrozen plan denying actions, a frozen plan allowing a
planned low-risk action, an unplanned branch being denied, replanning after
freeze being denied without changing branches, and invalid/duplicate/empty
plans failing closed.
