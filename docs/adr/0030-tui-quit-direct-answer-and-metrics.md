# ADR 0030: Guarded Quit, Direct Answers and Run Metrics

- Status: accepted for the interaction slice
- Date: 2026-11-09

## Context

A bare `1agent tui` starts a new session; reopening an earlier conversation is an
explicit `--session <id>` action. A single `Ctrl+Q` closed the TUI immediately; on
terminals or keymaps that report a control key unexpectedly, repeated Backspace
could therefore end the session. Direct Answer mode appended evidence/caveat
sections when the answer was not `Completed`, contradicting its conversation-first
contract. The running activity line only said `正在运行` and exposed no elapsed
time, token usage or provider cache statistics.

## Decision

1. A bare `1agent tui` starts a new session. `--session <id>` reopens an earlier
   session explicitly. The post-exit banner reports the current session title and
   continue command.
2. `Ctrl+Q` only arms an exit request. `Enter` confirms, `Esc` or other input
   cancels. The same rule applies while a run is active.
3. Direct Answer mode writes only the answer conclusion to the transcript for
   every answer status. Structured evidence, caveats and flip conditions remain
   available through `/details`; other modes keep their sections.
4. The running activity line is `Working... <elapsed> · ↑<input> ↓<output> ·
   cache <rate> · Ctrl+C 取消`. Elapsed time is derived from the run clock;
   token/cache metrics are refreshed from journaled provider responses and fixed
   at completion. Providers that do not report cache tokens show `cache —`.
5. `Usage` gains defaulted `cache_hit_tokens` and `cache_miss_tokens` fields.
   The OpenAI-compatible adapter reads OpenAI `prompt_tokens_details.cached_tokens`
   and DeepSeek `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens`.
6. PTY checks cover repeated Backspace and repeated Ctrl+Q without confirmation.

## Consequences

- A new launch remains a clean session; resuming an earlier conversation requires
  the explicit `--session <id>` command printed in the exit banner.
- Accidental or terminal-mapped quit keys no longer terminate the TUI; quitting
  takes one explicit confirmation.
- Direct answers stay concise even when the answer is conditional or needs
  evidence; structured detail is still auditable.
- The status line is wider and requires a terminal with enough columns; long
  values are clipped by the existing status layout rather than wrapping.
- Cache hit rate is honest: missing provider metadata is shown as unknown, not
  as 0%.
