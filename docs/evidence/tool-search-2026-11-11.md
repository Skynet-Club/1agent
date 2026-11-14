# Tool Search v0 — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `tool_search::search` and `1agent tools --query TEXT [--json]`. This is
a pure ranking function; it does not change the model-visible tool list.

## Commands

```bash
cargo test tool_search --lib
cargo test --test tool_search_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test tool_search::tests::exact_name_wins_and_read_matches_both_readers ... ok
test tool_search::tests::query_is_case_insensitive_and_uses_description ... ok
test tool_search::tests::empty_query_and_zero_limit_return_nothing ... ok
test tool_search::tests::limit_and_deterministic_ties_are_respected ... ok

test result: ok. 4 passed; 0 failed

running 2 tests (tests/tool_search_cli.rs)
test tool_search_ranks_name_and_description_matches ... ok
test tool_search_human_output_contains_score ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- `read` ranks `read_file` above `browser_read` (name prefix beats token
  match) and does not return `write_file`.
- `shell` matches `terminal_exec` through its description.
- Query matching is case-insensitive.
- Empty queries and zero limits return no hits; the result is capped.
- `tools --query` prints the same ranking as JSON or text with a score.

## What this does not prove

- That the model receives fewer tool definitions; request wiring is not done.
- That semantic synonyms work; the search is lexical.
- That the ranking improves task success; no model evaluation was run.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/tool_search.rs`
- `src/main.rs` (`tools --query`)
- `tests/tool_search_cli.rs`
- `docs/adr/0049-tool-search-v0.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
