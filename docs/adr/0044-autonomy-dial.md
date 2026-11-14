# ADR 0044: Autonomy Dial Policy

- Status: accepted
- Date: 2026-11-11

## Context

`UserModel` stores a per-domain autonomy preference (`observe`, `suggest`,
`act`, `auto`), but a preference is not an authorization grant. The runtime
needs one deterministic policy that combines the preference with risk,
reversibility and an explicit approval into an action decision.

Without that policy, each tool would invent its own interpretation of "auto".

## Decision

1. Add `src/autonomy.rs` with `decide(level, risk, reversible,
   explicit_approval)` returning one of:
   - `observe`: report only, do not propose an action;
   - `suggest`: propose and wait for the user;
   - `act`: execute and report;
   - `require_approval`: the action is ready but needs an explicit yes.
2. Rules:
   - `observe` and `suggest` never act, even with explicit approval;
   - `act` executes only low-risk reversible actions; medium/high risk or
     irreversible actions require approval;
   - `auto` executes low/medium-risk reversible actions; high risk or
     irreversible actions require approval;
   - explicit approval covers `act` and `auto`, including high-risk or
     irreversible actions;
   - an action with no recorded compensation is irreversible and therefore
     requires approval unless explicitly approved.
3. `decide_for(model, domain, ...)` reads `UserModel::autonomy_for(domain)`.
   An unset domain defaults to `suggest`.
4. Add `1agent autonomy --level ... --risk ... [--reversible]
   [--explicit-approval] [--json]` for inspection.
5. This is a policy module, not an execution path. Authority, Policy and
   ExecutionGate still enforce the final decision.

## Consequences

- The same preference produces the same decision across tools.
- `auto` is not "always act"; high risk and irreversible actions still need a
  human.
- Explicit approval is scoped to the action being decided, not a blanket
  permission.
- A future UI can explain the decision in the user's language using the
  `reason` field.

## Evidence

```bash
cargo test autonomy --lib
cargo test --test autonomy_cli
```

The unit tests cover observe/suggest, act vs auto, high risk, irreversible
actions, explicit approval and per-domain lookup. The CLI test covers the
decision outputs and invalid level/risk failures.
