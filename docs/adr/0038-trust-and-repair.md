# ADR 0038: Trust & Repair — Side-Effect Repair Plans and Away Digest

- Status: accepted
- Date: 2026-11-11

## Context

The side-effect ledger records `planned`, `applied`, `compensated` and
`failed` actions, but the user had no single view answering two questions:

1. "What happened while I was away?"
2. "Can this action be undone, and if not, who must decide?"

An action that succeeds is not automatically trustworthy. A reversible action
without a recorded compensation is a bug; an irreversible action needs a human
decision; a failed action may have left a partial effect. These are repair
states, not just execution states.

## Decision

1. Add `src/repair.rs`.
2. `plan_repair(effect)` classifies a side effect:
   - `planned` or `compensated` → `none` (nothing to undo);
   - `applied` + reversible + compensation → `compensate`;
   - `applied` + reversible + **no** compensation → error (fail closed);
   - `applied` + irreversible → `manual`;
   - `failed` → `manual` (verify no partial effect remains).
3. `receipt(effect)` produces a user-facing `TrustReceipt` with the action,
   target, status, idempotency key and undo instruction.
4. `build_away_digest(effects, since, until)` partitions the bounded window
   into applied, compensated, failed, pending, `needs_user` and receipts.
5. `1agent away-digest --store ... [--since-ms ...] [--until-ms ...] [--json]`
   exposes the view. Default window is the last 24 hours.
6. This slice **plans** compensation; it does not execute it. Execution must
   still pass Authority/Policy/Approval and is a later increment.
7. Repair is not a new source of truth: it is a projection over the side-effect
   ledger, so it can be rebuilt from replay.

## Consequences

- A user can see what changed, what was undone and what needs a decision.
- A reversible action without a compensation fails closed instead of silently
  becoming irreversible.
- An irreversible action is never presented as automatically repairable.
- The away digest is deterministic for a fixed ledger and time window.

## Evidence

```bash
cargo test repair --lib
cargo test --test away_digest_cli
```

The unit tests cover planned/compensated, reversible compensation,
missing compensation, irreversible manual review, failed cleanup, window
filtering and receipt fields. The CLI test is added when a task-graph fixture
with side effects is available.
