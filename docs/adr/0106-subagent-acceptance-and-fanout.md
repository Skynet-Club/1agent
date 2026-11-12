# ADR 0106: Subagent Parent Acceptance and Fanout

- Status: accepted
- Date: 2026-11-12
- Related: SA99-59

## Context

`spawn_agent` returned a child's conclusion whenever the child process exited
zero and printed JSON. A child that stopped at `needs_authorization` or
`needs_evidence` could therefore look successful to the parent. The tool also
had a depth limit and permission intersection but no fanout bound: one parent
could spawn an unbounded number of children sequentially.

## Decision

1. The parent accepts a child only when `stop_reason == "completed"`.
   Any other stop reason returns a failed tool observation with the reason and
   the child's first conclusion line.
2. If the child reports a journal path and that file exists, the parent checks
   it is at most 16 MiB, reads it, and requires `audit_events` to pass. A
   missing journal is not audited; this is a limitation, not a success claim.
3. `SpawnAgentTool` has a per-instance total fanout limit of 8 children. The
   counter never decrements, so it bounds total spawns for that tool instance,
   not just concurrent ones.
4. Depth (2), permission intersection, timeout and step limits are unchanged.

## Consequences

- A non-completed child can no longer be presented to the parent as a
  successful subtask.
- A child with a corrupt or unauditable journal is rejected when the journal
  is present.
- The fanout bound is process-local. A fresh parent process gets a fresh
  budget; coordinated fanout across processes is not bounded.
- A child that omits its journal path is accepted on `completed` alone; the
  tool does not fabricate a journal.
