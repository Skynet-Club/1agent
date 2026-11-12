# Subagent Acceptance and Fanout — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `SpawnAgentTool` parent acceptance and fanout bound.

## Commands

```bash
cargo test --offline tools::subagent --lib
```

## What this proves

- A child whose stop reason is `needs_authorization` is rejected by the
  parent with a failed tool observation naming the reason.
- A child that reports no journal can still pass on `completed` (the fake
  program path); a present journal is size-checked and audited.
- After eight spawns from one tool instance, the ninth returns a fanout-limit
  failure.
- Existing depth, workspace containment and permission-intersection tests
  still pass.

## What this does not prove

- Real subagent journal auditing; the fake program reports no journal.
- Cross-process or cross-run fanout bounds; the counter is per tool instance.
- That `completed` means the child's work is correct; it only means the child
  run terminated in the expected state.
