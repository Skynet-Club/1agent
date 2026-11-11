# ADR 0050: Wire Tool Search into Request Construction

- Status: accepted
- Date: 2026-11-11

## Context

ADR-0049 added a pure `tool_search` ranking and `tools --query`, but every run
still sent the full tool list to the model. The next step is to let the caller
select a smaller advertised set without losing the basic file/terminal tools.

## Decision

1. Add `AgentRuntime::with_tool_query(query)`.
2. When a query is set, the runtime advertises:
   - the top 8 search hits for the query;
   - plus the always-on tools `read_file`, `write_file`, `edit_file` and
     `terminal_exec` when they are available under the current permissions.
3. With no query, behavior is unchanged: the full permission-filtered list.
4. Add `1agent run --tool-query TEXT`.
5. The selected list is what is journaled in `ModelRequested`, so a run can be
   audited for which tools the model actually saw.
6. This is still lexical search; there is no semantic retrieval and no
   "search for a tool" meta-tool. The always-on set is deliberately small.

## Consequences

- A run can reduce tool-definition context while keeping basic file/terminal
  work available.
- A query that misses a needed non-core tool will hide it; the user can run
  without `--tool-query` or use a broader query.
- The selection is deterministic and journaled, not a hidden model-side choice.
- A later slice can add a discovery meta-tool or progressive loading.

## Evidence

```bash
cargo test tool_search --lib
cargo test --test tool_query_cli
```

The unit tests cover selection and always-on tools; the CLI test asserts the
journaled request contains `terminal_exec` and not `browser_read` for a
`shell` query, while a run without the query still contains `browser_read`.
