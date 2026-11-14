# ADR 0114: Journal-Derived Stage Latencies

- Status: accepted
- Date: 2026-11-12
- Related: SA97-01

## Context

SA97-01 requires route, context assembly, model, tool, journal-write and TUI
render durations to be readable from a run. Every `JournalEvent` already carries
`timestamp_unix_ms`, and the runtime already journals the boundaries for four of
those stages. Adding new timing events would change the journal contract and
risk double-counting; projecting from existing timestamps is deterministic and
works on old journals.

## Decision

1. `metrics::JournalMetricsReport` gains `route_latency`, `context_latency`,
   `model_latency` and `tool_latency` digests.
2. Stages are disjoint:
   - route: run/resume start to the first `RouteSelected`;
   - context: first `RouteSelected` to `ModelRequested`, then the previous
     `ModelResponded` or `ToolObserved` to the next `ModelRequested`;
   - model: `ModelRequested` to `ModelResponded` or `ProviderFailed`;
   - tool: `ToolCalled` to the matching `ToolObserved` by `call_id`.
3. Retries add model samples but not context samples, so provider backoff is
   not misreported as context assembly.
4. The text and JSON `1agent metrics` outputs expose the four digests. A
   missing boundary or a non-monotonic timestamp produces no sample rather than
   a fabricated zero.
5. Journal-write and TUI-render timing are not inferred from event gaps; they
   remain covered by the existing write path and the TUI frame probe. Adding
   per-append timing events would change journal volume and is deferred.

## Consequences

- Stage timings are available offline from existing journals with no runtime
  behavior change.
- Resolution is one millisecond and a crashed mid-call stage has no sample.
- The projection is structural: it measures wall-clock gaps between journaled
  boundaries, not CPU time or provider-internal latency.
- Journal write and TUI render still need dedicated probes before SA97-01 can
  claim all six stages.
