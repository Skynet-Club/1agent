# ADR 0030: TUI Session Resume, Guarded Quit and Run Metrics

- Status: accepted for the interaction slice
- Date: 2026-11-09

## Context

A bare `1agent tui` always created a new session ID, so quitting and relaunching
lost the previous conversation even though journals and session metadata were
present. A single `Ctrl+Q` closed the TUI immediately; on terminals or keymaps
that report a control key unexpectedly, repeated Backspace could therefore end
the session. Direct Answer mode appended evidence/caveat sections when the answer
was not `Completed`, contradicting its conversation-first contract. The running
activity line only said `正在运行` and exposed no elapsed time, token usage or
provider cache statistics.

## Decision

1. A bare `1agent tui` resumes the most recently active non-archived session that
   has at least one run; `--session <id>` remains an explicit override. Session
   ordering uses the newest of `session.json` and `events.jsonl` mtimes.
2. `Ctrl+Q` only arms an exit request. `Enter` confirms, `Esc` or other input
   cancels. The same rule applies while a run is active. The post-exit banner
   states that `1agent tui` restores the recent session.
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
6. PTY checks cover repeated Backspace, repeated Ctrl+Q without confirmation, and
   a bare TUI relaunch that must display the previous session.

## Consequences

- Session history is visible after a normal quit without copying a session ID;
  archived sessions are skipped.
- Accidental or terminal-mapped quit keys no longer terminate the TUI; quitting
  takes one explicit confirmation.
- Direct answers stay concise even when the answer is conditional or needs
  evidence; structured detail is still auditable.
- The status line is wider and requires a terminal with enough columns; long
  values are clipped by the existing status layout rather than wrapping.
- Cache hit rate is honest: missing provider metadata is shown as unknown, not
  as 0%.
