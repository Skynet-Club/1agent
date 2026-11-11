# ADR 0053: Computer-Use Provenance and Branch-Steering Policy

- Status: accepted
- Date: 2026-11-12

## Context

Computer-use agents read untrusted screen content and then act. Prompt
injection can arrive through a fake button, hidden DOM or a value copied from a
page. The 2026 research consensus is that prompt-level defenses are not enough:
the planner must not see untrusted content, and an action's arguments must carry
their provenance.

The browser/CUA implementation is not part of this slice. The policy contract is.

## Decision

1. Add `src/cua.rs` with `ArgumentSource`:
   - `plan`: a constant from the approved plan;
   - `trusted_observation`: a trusted local/config value;
   - `untrusted_observation`: a value from a page, screenshot or other
     untrusted content.
2. `evaluate(action)`:
   - empty action name → `deny`;
   - high-risk action with any untrusted argument → `deny`;
   - any untrusted argument → `require_approval`;
   - irreversible action above low risk → `require_approval`;
   - otherwise → `allow`.
3. `planner_sees_untrusted(true)` → `deny`. The privileged planner may only
   see plan/config state; a quarantined perception layer resolves runtime
   values.
4. `check_branch(target, planned_branches)` → `allow` only when the target is
   in the pre-approved plan. This blocks branch steering to an unplanned
   "valid but dangerous" path.
5. Add `1agent cua-check --action ... --risk ... [--untrusted]
   [--reversible] [--branch ... --planned-branch ...] [--json]` for
   inspection.
6. This is a policy module. No browser automation, screenshot handling or
   execution is implemented here.

## Consequences

- A future CUA tool must pass provenance and branch checks before acting.
- Untrusted arguments are not silently trusted; high-risk ones are denied.
- The policy is testable without a browser or model.
- It does not prove a real page cannot steer the agent; it defines the
  contract the CUA implementation must satisfy.

## Evidence

```bash
cargo test cua --lib
cargo test --test cua_check_cli
```

The tests cover trusted/untrusted arguments, risk levels, irreversible
actions, planner isolation and planned/unplanned branch targets.
