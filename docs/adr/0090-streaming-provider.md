# ADR 0090: Streaming Provider Deltas

- Status: accepted
- Date: 2026-11-12

## Context

The OpenAI-compatible adapter waited for the complete HTTP body before the
runtime could show or journal anything. Long generations therefore looked
stalled, and the user could not see the first token. The runtime already
supports cancellation and provider retries, but a streaming implementation
must not break those paths or silently mix partial output from two attempts.

## Decision

1. `ModelProvider` gains `complete_streaming(request, cancellation, on_text)`.
   Its default implementation delegates to `complete` and never calls
   `on_text`, so deterministic and third-party providers keep working.
2. `OpenAiCompatibleProvider` sends `"stream": true`, parses server-sent
   events, reports each content fragment through `on_text`, and accumulates
   tool-call fragments by index. Tool calls are returned only when the stream
   ends, so a half call is never observed.
3. SSE lines are reassembled across network chunks. `data:` payloads are
   parsed as JSON; `[DONE]`, comments and non-`data` fields are ignored. An
   `error` chunk fails closed. The existing response byte limit applies to the
   whole stream.
4. Cancellation is checked while waiting for the response and between chunks,
   and returns `ProviderErrorCode::Cancelled`.
5. `AgentRuntime::with_stream_sink` installs an optional observer. Only the
   first provider attempt streams; retries use `complete`, so a failed attempt
   cannot duplicate or interleave partial text with the fallback answer.
6. `1agent run --stream` prints content fragments to stderr and flushes each
   fragment. Stdout remains the final JSON or rendered answer.

## Consequences

- A caller can see the first provider token before the run finishes, and the
  same cancellation token stops both the HTTP stream and the run.
- Journal contents are unchanged: the final `ModelResponse` is still the one
  complete response, so replay and audit do not depend on deltas.
- `--stream` fragments are presentation only and are not a second evidence
  source. They are not redacted beyond what the provider sends; the final
  answer remains the authoritative output.
- The TUI does not consume the sink yet; it keeps its current non-streaming
  presentation until it has a render path that can merge deltas.
- Providers that reject `stream: true` fail with the provider's error. The
  CLI flag is opt-in; without it the adapter uses the non-streaming path.
- Streaming usage is whatever the provider reports after the deltas. A
  provider that omits usage makes token/cost budgets undercount, so `--stream`
  is opt-in and must not be treated as a billing-grade measurement.
