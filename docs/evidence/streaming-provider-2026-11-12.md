# Streaming Provider — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `ModelProvider::complete_streaming`, the OpenAI-compatible SSE path,
runtime stream sink and `1agent run --stream`.

## Commands

```bash
cargo test stream --lib
cargo test --test stream_cli
cargo test --test openai_adapter -- --ignored
```

## Results

```text
provider::tests::stream_lines_accumulate_text_usage_and_tool_calls ... ok
provider::tests::stream_error_chunk_fails_closed ... ok
provider::tests::streaming_falls_back_to_complete_when_unimplemented ... ok
runtime::tests::stream_sink_receives_deltas_before_the_final_answer ... ok
runtime::tests::retry_after_partial_stream_does_not_replay_or_stream_the_fallback ... ok
stream_cli::run_stream_prints_provider_deltas_before_the_final_json ... ok
openai_adapter::openai_compatible_adapter_streams_text_deltas ... ok
openai_adapter::openai_compatible_adapter_streams_tool_call_deltas ... ok
openai_adapter::openai_compatible_adapter_cancels_a_stream_after_first_delta ... ok
```

## What this proves

- The CLI prints content fragments to stderr before the final JSON on stdout,
  and the final answer still parses as `RunResult`.
- The OpenAI-compatible adapter sends `"stream": true`, reassembles SSE lines
  split across TCP writes, reports each content fragment in order, accumulates
  usage, and assembles fragmented tool-call arguments into one valid call.
- Cancelling after the first fragment returns `Cancelled` without waiting for
  the server to finish.
- A provider that implements only `complete` still works through
  `complete_streaming`; the callback is never called.
- A retryable failure after a partial stream does not stream the fallback and
  does not duplicate the partial fragment; the fallback answer completes and
  the journal audit still passes.
- An `error` SSE chunk fails closed with `InvalidResponse`.

## What this does not prove

- TUI streaming; the TUI does not consume the sink yet.
- That a provider accepts `stream: true`; the flag is opt-in and a rejecting
  provider surfaces its HTTP error.
- Redaction or trust of streamed fragments; deltas are presentation only.
- Multi-line `data:` events; one JSON payload per `data:` line is assumed.
- Real-provider behavior and latency; tests use a loopback fake server.
- Billing-grade usage: streaming usage is only what the provider reports. A
  provider that omits usage after the deltas makes token/cost budgets
  undercount, so `--stream` is opt-in and not a billing measurement.
