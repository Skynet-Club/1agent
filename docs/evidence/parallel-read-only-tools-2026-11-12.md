# Parallel Read-Only Tools — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `AgentTool::parallel_safe`, batch grouping, concurrent read-only
execution and journal ordering.

## Commands

```bash
cargo test only_side_effect_free_tools --lib
cargo test tool_groups --lib
cargo test parallel_safe_tool_calls --lib
cargo test write_tools_do_not_overlap --lib
```

## What this proves

- Only `read_file`, `search_files`, `read_pdf`, `fetch_url`, `web_search` and
  `browser_read` are parallel-safe; write, terminal, skill, subagent and MCP
  tools are not. An unknown tool is not parallel-safe.
- A batch `[read, read, write, read]` is grouped as `[(0,2), (2,3), (3,4)]`,
  so the write stays an ordered boundary.
- Two read-only calls each sleep 120 ms and 10 ms; both overlap (peak
  concurrency 2), and observations still appear in model order even though the
  second call finishes first. The journal audit passes.
- A read, a write and a read never overlap (peak concurrency 1), and the
  journal observes `call-1`, `call-2`, `call-3` in order.

## What this does not prove

- Real network or browser latency improvements; the timing tool is local.
- Any global concurrency cap; a model batch with many reads can start them all.
- Recovery of an interrupted concurrent group; unpaired calls still block
  `resume`.
- That a third-party MCP tool is safe to parallelize; MCP tools stay ordered
  unless a future explicit capability says otherwise.
