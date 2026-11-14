# Status v0.93.0

Date: 2026-11-03

## Implemented

- Deterministic goal triage and bounded model planning.
- Strict Work Item DAG, risk, permission, path and Artifact validation.
- Capability/tool/availability/write/risk-based Agent assignment.
- Plan approval, rejection and versioned revision before execution.
- Persistent task leases, idempotency keys and stale-writer rejection.
- Workshop Agent Registry, historical Job and unified Event reads.
- Controlled synchronous and detached persistent-Session execution.
- `master poll` progression for running, blocked, timed-out and terminal execution.
- Explicit `[1A:AWAITING_INPUT]`, `[1A:RUNNING]` and `[1A:BLOCKED]` progress markers.
- Human takeover, pause/resume, cancellation, one retry and one scoped replan.
- Independent path-confined Artifact verification and streaming SHA-256.
- Hash-chained Master Journal replay and audit.
- Private planner request/response Journal with one structured-output repair.

## Current evidence

- 187 library tests, 4 main/TUI tests and 8 CLI E2E tests pass.
- A real Workshop Session completed, was verified and replayed with a valid hash chain.
- A detached real Session was observed as running before terminal convergence.
- A real exit-zero/missing-Artifact pilot was rejected.
- DeepSeek generated a valid, unapproved, high-risk-aware two-item plan.
- Workshop currently exposes 13 Agent records and 7 historical Jobs to the Adapter.

## Blocked measurements

Pi, Hermes, Claude and Codex have not completed the same-task capability suite in
this environment. See `docs/master-capability-matrix-v1.json`.

## Start here

See `docs/master-agent.md` and `docs/handoff-v0.93.md`.
