# ADR 0105: First-Token Instrumentation

- Status: accepted
- Date: 2026-11-12
- Related: M7 TTFT, SA97-08

## Context

`run --stream` reports content fragments, but the journal had no first-chunk
timestamp. M7 (TTFT p50/p95) could not be computed from runs.

## Decision

1. For the first provider attempt, the runtime wraps the configured stream
   sink. The wrapper stores the elapsed time from just before the provider
   call to the first non-empty content fragment.
2. After the provider call returns, if a fragment was seen, the runtime
   journals `StreamFirstToken { provider_id, model, elapsed_ms }`.
3. `JournalMetricsReport` gains `ttft_latency` (p50/p95/max/count) from all
   `StreamFirstToken` events; `1agent metrics` prints it when present.
4. The event is emitted only when a stream sink is configured (`--stream`).
   A non-streaming run has no TTFT event rather than a synthetic zero.

## Consequences

- M7 is measurable for streaming runs; the elapsed value is the real
  first-fragment latency even though the event is appended after the response
  completes.
- A retry or fallback attempt emits its own event; metrics include every
  attempt, not only the successful one. This is intentional for M7 (latency
  distribution), but it is not a per-run “time to first answer” metric.
- Runs without `--stream` have no samples; M7 coverage depends on how often
  streaming is enabled.
- The event is presentation/latency metadata, not evidence; it does not change
  answer validation or authority.
