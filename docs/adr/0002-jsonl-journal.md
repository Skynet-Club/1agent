# ADR 0002: JSONL Run Journal

- Status: accepted
- Date: 2026-11-01

## Context

The agent must replay and audit model-visible inputs, evidence, tool calls, observations, routing and stop reasons. A database would add platform and migration cost before the event contract is stable.

## Decision

Use an append-only JSONL journal with versioned envelopes and contiguous sequence numbers. A deterministic projection reconstructs run state. The storage interface can later be replaced without changing event semantics.

## Consequences

- Human-readable, portable and easy to test.
- Requires explicit recovery handling for partial trailing lines.
- Does not provide database indexing, concurrency or encryption; those remain later adapters.
