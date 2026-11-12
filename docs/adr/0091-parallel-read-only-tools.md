# ADR 0091: Parallel Read-Only Tools

- Status: accepted
- Date: 2026-11-12

## Context

The runtime executed every tool call sequentially. Model batches that read
several files, fetch several pages or search several roots therefore paid the
sum of their latencies. The journal contract requires deterministic order and
paired call/observation entries, and write or terminal tools must keep their
side effects ordered.

## Decision

1. `AgentTool::parallel_safe` defaults to `false`. Only side-effect-free tools
   opt in: `read_file`, `search_files`, `read_pdf`, `fetch_url`, `web_search`
   and `browser_read`. Write, terminal, delegation, skill, subagent and MCP
   tools remain ordered.
2. A model tool batch is split into contiguous groups: consecutive
   parallel-safe calls form one concurrent group, every other call is its own
   group. Groups run in model order, so a read before a write still happens
   before the write.
3. Within a group, `ToolCalled` events are appended in model order before
   execution, calls run concurrently, and `ToolObserved` events are appended in
   the same model order regardless of completion order. The journal therefore
   stays deterministic and every call is paired.
4. The existing per-tool timeout and cancellation token apply to each
   concurrent call. The tool-call budget is checked before the batch.

## Consequences

- Multiple independent reads overlap, while writes and shared-state tools stay
  sequential.
- Replay and audit do not depend on scheduling: repeated runs of the same
  deterministic tool batch produce the same journal order.
- An interrupted concurrent group leaves unpaired calls, so checkpoint resume
  refuses it. This is the same conservative behavior as an interrupted
  sequential call: re-running writes could duplicate side effects.
- `join_all` runs the group on the runtime task rather than spawning separate
  tasks, so a tool panic still propagates like a sequential tool panic.
- Concurrency is bounded only by the number of calls in one model batch; there
  is no separate global semaphore yet.
