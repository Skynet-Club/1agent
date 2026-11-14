# ADR 0049: Tool Search v0

- Status: accepted
- Date: 2026-11-11

## Context

As the tool surface grows (files, web, browser, terminal, MCP, subagents,
skills), sending every tool definition to the model on every request wastes
context and makes tool choice harder. The 2026 harness practice is to load
tool definitions on demand.

A full on-demand loader changes the model request contract. This v0 slice
starts with the pure ranking function and an operator-visible search command.

## Decision

1. Add `src/tool_search.rs` with `search(query, specs, limit)`.
2. Ranking is deterministic and local:
   - exact name match: +100;
   - name prefix match (`read` → `read_file`): +30;
   - name token match: +20;
   - description token match: +5;
   - parameter-name token match: +2;
   - ties break by tool name.
3. Tokens are lowercase alphanumeric runs; matching is case-insensitive.
4. Empty query or zero limit returns no hits; the result is capped by `limit`.
5. Add `1agent tools --query TEXT [--json]`. Without `--query`, the existing
   capability list is unchanged.
6. This does **not** change what the model can call or what the registry
   advertises. Wiring search into request construction is the next slice.

## Consequences

- An operator can inspect which tools are relevant to a query.
- The ranking is testable without a model or network.
- A later loader can reuse the same ranking and add a small always-on tool set.
- The search is lexical, not semantic; synonyms may miss.

## Evidence

```bash
cargo test tool_search --lib
cargo test --test tool_search_cli
```

The tests cover exact/prefix/description matching, case-insensitivity, empty
queries and deterministic limits.
