# Tool Search Request Wiring — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `tool_search::select`, `AgentRuntime::with_tool_query` and
`1agent run --tool-query`. The selected tool list is journaled in
`ModelRequested`.

## Commands

```bash
cargo test tool_search --lib
cargo test --test tool_query_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 6 tests (tool_search)
...
test result: ok

running 1 test (tests/tool_query_cli.rs)
test tool_query_limits_the_journaled_tool_list ... ok

test result: ok. 1 passed; 0 failed
```

## What this proves

- `select("shell", ...)` returns `terminal_exec` plus the always-on
  `read_file`, `write_file` and `edit_file`, and drops `browser_read`.
- An empty query returns the original list.
- A run with `--tool-query shell` journals `terminal_exec` and not
  `browser_read`.
- A run without the query journals `browser_read`.
- The selection is deterministic and visible in the journal.

## What this does not prove

- That the model chooses better tools or completes tasks faster; no model
  evaluation was run.
- That semantic/ synonym queries work; the search is lexical.
- That a missed non-core tool is recoverable; the user must broaden the query
  or run without it.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/tool_search.rs` (`select`, always-on set)
- `src/runtime.rs` (`tool_query`, `with_tool_query`)
- `src/main.rs` (`run --tool-query`)
- `tests/tool_query_cli.rs`
- `docs/adr/0050-tool-search-request-wiring.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
