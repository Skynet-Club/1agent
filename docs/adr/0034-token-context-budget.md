# ADR 0034: Token-Based Context Budget for 1M-Token Sessions

- Status: accepted
- Date: 2026-11-10

## Context

1Agent measured context in characters: a 100,000-character budget with automatic
compaction at 80%. The TUI could not be compared with a 1M-token coding session, whose current
session uses a 1,000,000-token window and compacts at 786,432 tokens. Character
counts also made model/provider limits hard to reason about, because a Chinese
character and an ASCII character consume different numbers of tokens.

The user asked 1Agent to align with a 1M-token coding session: 1M tokens maximum, automatic
compaction at 786,432 tokens, and a full migration from character semantics.

## Decision

1. `RunBudget` stores `max_context_tokens` (default 1,000,000) and
   `auto_compact_tokens` (default 786,432). Session preferences use the same
   units and bounds (1,000 to 1,000,000 tokens, compaction at or below the
   maximum).
2. Runtime compaction selects old tool-result payloads using a token estimate
   and triggers at `auto_compact_tokens`.
3. Before a request, tokens are estimated provider-neutrally: ASCII text at four
   characters per token, every non-ASCII character as one token, plus message
   and tool-call overhead. After a response, the provider-reported
   `usage.input_tokens` is authoritative and replaces the estimate in the TUI.
4. The TUI status card, `/context show`, `/status` and `/config` report tokens.
   `--max-context-chars` is replaced by `--max-context-tokens`.
5. Old task/session JSON is migrated on read:
   - the old 100,000-character default maps to the new 1,000,000-token default;
   - other character budgets map to `ceil(chars / 3)` tokens;
   - old 80% default compaction maps to 786,432 tokens;
   - other percentages map to `max_tokens * percent / 100`.
   New serialized data contains only token fields.
6. `ContextCompacted` journals carry both token and character counts. Token
   fields default to zero for old journals; readers fall back to the character
   fields when both token fields are zero.

## Consequences

- 1Agent's budget now matches a 1M-token coding session: 1M tokens with
  compaction at 768K (786,432).
- Request-time token counts are estimates. They can differ from provider usage,
  especially for mixed-language content; each response recalibrates the display.
- Compaction is still a deterministic pruning of old tool results, not an
  LLM-generated summary. `/context compact` remains the explicit local summary.
- Old configurations are readable, but new fields are the only ones written.
  The CLI flag rename is breaking for external scripts.
- Tests cover token estimation, token-triggered compaction, legacy migration and
  the TUI token display.
