# ADR 0035: User Model 2.0 — Autonomy, Voice, and Memory Consent

- Status: accepted
- Date: 2026-11-11

## Context

`UserModel` was a read-only view over scoped memory records: preferences and
boundaries, each keeping the memory id it came from. A partner needs more than
preferences: how much it may do without asking, how it should speak, and what it
may remember. These are relationship settings, not execution authority.

The runtime already has authority/policy/approval and memory scope/taint/TTL.
Adding a second settings store would split the source of truth and make replay
harder. The model must remain rebuildable from memory journal events.

## Decision

1. `UserModel` remains a **derived view**, not a second source of truth.
2. Add `autonomy.<domain>` preferences. Values are `observe`, `suggest`, `act`,
   `auto`. Unset domains default to `suggest`; a new domain never starts at
   `auto`.
3. Add `voice` / `voice_mode` preferences. Values are `operator`, `advisor`,
   `companion`. Unset defaults to `operator`.
4. Add feedback records shaped `consent:<kind>=allow|deny[@<retention_ms>]`.
   `<kind>` is a memory kind (`fact`, `preference`, `decision`, `project`,
   `feedback`, `relationship`, `skill_hint`). Default is allow; consent rules
   opt out, they do not opt in. The latest record per kind wins.
5. New `UserModel` fields use `#[serde(default)]` so older serialized models
   keep deserializing.
6. Add `1agent user-model` to print the view as JSON or human-readable text.
7. Autonomy and consent are **preferences**, not grants. Authority, Policy,
   ExecutionGate and Approval still decide whether an action is allowed.
   Continuity, delegation and memory rules must not bypass them.

## Consequences

- A partner can be configured per domain without a new database.
- Memory consent is auditable: every rule points at the memory record that set
  it, and forgetting the record removes the rule on rebuild.
- The CLI exposes the relationship state for debugging and user review.
- A future UI can edit these preferences by writing memory records through the
  existing `MemoryService`; no separate migration is required.
- Invalid or forgotten records are ignored, so a stale rule cannot silently
  grant more autonomy.

## Evidence

```bash
cargo test user_model --lib
cargo test --test user_model_cli
```

The tests cover parsing, newest-wins ordering, forgotten records, invalid
values, safe defaults, serialization round-trip and the CLI JSON/human output.
