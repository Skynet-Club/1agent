# ADR 0047: Agent Card v0

- Status: accepted
- Date: 2026-11-11

## Context

External tools, collaborators and auditors need a machine-readable statement of
what this build supports, what it only partially supports and what it does not
claim. Without one, capability claims drift into README prose and are hard to
compare across versions.

## Decision

1. Add `src/agent_card.rs` with `AgentCard`:
   - `schema_version`, `name`, `version`, `description`, `platform`;
   - `capabilities: Vec<CardCapability { id, status, note }>`;
   - `limits: Vec<String>`.
2. Capability statuses reuse `platform::CapabilityStatus`
   (`verified` / `degraded` / `unsupported` / `untested`).
3. Core capabilities (`task_evidence_audit`, `user_model`, `agent_inbox`,
   `autonomy_dial`, `voice_modes`, `onboarding`, `value_report`,
   `independent_verification`, `repair`) are marked `verified` because they
   have contract tests in this repository.
4. Platform-constrained capabilities (`computer_use`, `pty`) take their status
   from the platform registry, not from prose.
5. `cross_platform` is `untested`; `team_collaboration` and `marketplace` are
   `unsupported`. The card must not mark them verified.
6. `1agent agent-card [--json]` prints the card.
7. This is a local, unsigned card. It is not yet an A2A Agent Card served over
   HTTP or signed by a registry; those are later slices.

## Consequences

- A reader can compare builds without parsing README claims.
- Unsupported or untested capabilities are explicit and testable.
- The card can later be served by the Surface API or signed for A2A without
  changing the capability vocabulary.

## Evidence

```bash
cargo test agent_card --lib
cargo test --test agent_card_cli
```

The tests cover required fields, JSON round-trip, unsupported/untested status
rules and both CLI output modes.
