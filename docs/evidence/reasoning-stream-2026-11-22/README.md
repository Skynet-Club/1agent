# Reasoning part live check (2026-11-22)

Goal: make model reasoning a first-class part — streamed in the TUI and stored
in the conversation session, without replaying it into later model context.

## Change

- `deepseek-flash` does emit `reasoning_content`. The OpenAI-compatible stream
  parser now reports those fragments through a separate `on_reasoning`
  callback; content and tool calls keep their existing path.
- `ModelProvider` gains `complete_streaming_with`; the default ignores
  reasoning and keeps the old streaming behavior, so other providers are
  unchanged.
- `AgentRuntime::with_reasoning_sink` receives the fragments. The first
  attempt passes them to the TUI and appends one full text to the session after
  the response; retries stay non-streaming.
- `SessionEntry::Reasoning { run_id, step, text }` is stored on disk but
  skipped by `context_messages`; `needs_resume_notice` ignores reasoning so a
  crash after reasoning still gets a resume notice.
- The TUI drains the sink every 120ms into a single `模型思考 Thinking` row.
  The live row is capped at 8,000 characters; the session keeps the full text.

## Live check

Installed binary `d8f5112a…`, real DeepSeek, TUI through tmux. Workspace
`/tmp/tui-reasoning-final`, session `ses_tui_reasoning_final_20261122`.

### Turn 1

Task: `写一个比较：9.11 和 9.9 哪个大？请说明理由，并检查一个常见错误。`

Result: 1 model call, 8.1s wall, `completed`. The session contains a
`reasoning` entry of **2,247 characters** followed by the assistant answer.
`pane-1s.txt` and `pane-3s.txt` were captured while the status was still
running and show partial reasoning under `模型思考 Thinking`, for example:

```
 • 模型思考 Thinking · This is a pure math/reasoning question. 9.9 > 9.11. Answ
   er directly, no tools needed.
   The common error: treating decimals like version numbers ...
```

### Follow-up

Task: `我刚才问了什么？只回答一句。`

Result: 1 model call, `completed`, 9 request messages. The session now has two
reasoning entries (2,247 + 453 characters). The follow-up request contains the
prior user question and the prior assistant answer, but **neither full
reasoning text**. Reasoning is stored, not replayed.

## Tests

- `stream_lines_accumulate_text_usage_and_tool_calls`: reasoning fragments are
  captured separately from content and tool calls.
- `reasoning_streams_to_the_sink_and_the_session`: a streaming provider's
  reasoning reaches the sink and becomes a session entry; the entry is absent
  from projected context.
- `reasoning_is_stored_but_not_projected_into_context` and the resume-notice
  assertion: reasoning is skipped by context projection and does not hide an
  unanswered user turn.
- `reasoning_fragments_share_one_live_row` and
  `reasoning_buffer_is_shared_with_the_runtime_sink`: the TUI keeps one live
  Thinking row and shares its buffer with the runtime.
- Gates: lib 1428/0/4, bin suite exit 0, fmt, strict clippy, eval 8/8.

## Limits

- Only providers that emit streamed `reasoning_content` produce this part;
  non-streaming `complete` calls ignore it.
- Only runs with a reasoning sink capture it: the TUI does; the CLI `run`
  without streaming does not.
- Reasoning is stored in the conversation session, not the per-run journal.
  The journal remains the model input audit; reasoning is model output.
- The live TUI row is capped at 8,000 characters; the session has the full
  text.
- Reasoning is never replayed into model context by design.
