# ADR 0056: OpenTelemetry GenAI-Style Span Projection

- Status: accepted
- Date: 2026-11-12

## Context

Enterprises want agent traces in their existing APM. The run journal already
contains the facts: run start/stop, model requests/responses with usage, and
tool calls/observations. Adding a full OTel SDK is unnecessary for a local
runtime; a stable JSON span projection is enough for a host to forward.

## Decision

1. Add `src/otel.rs` with `OtelSpan`, `SpanKind`, `SpanStatus` and
   `spans_from_events(events)`.
2. Emit:
   - one root `invoke_agent 1Agent` span per run;
   - one `chat <provider>/<model>` span per model response, with
     `gen_ai.operation.name`, `gen_ai.provider.name`, `gen_ai.request.model`,
     `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`,
     `gen_ai.usage.cost_micros` and `gen_ai.conversation.id`;
   - one `execute_tool <tool>` span per tool observation, with
     `gen_ai.tool.name`, `gen_ai.tool.call_id` and `gen_ai.tool.status`.
3. Span and trace IDs are deterministic functions of the run id and event
   order, so a replay produces the same projection.
4. Run status maps `completed` to `ok`, needs-authorization/evidence to
   `unset`, and other stop reasons to `error`.
5. Add `1agent otel --journal PATH [--json]`.
6. This is a projection, not an OTel wire implementation: no exporter,
   propagation, metrics or logs.

## Consequences

- A host can forward JSON spans to an OTel collector without changing the
  journal contract.
- Attribute names follow the GenAI conventions closely enough for dashboards.
- The projection is deterministic and testable offline.
- It does not provide distributed trace context across processes or full
  semantic-convention coverage.

## Evidence

```bash
cargo test otel --lib
cargo test --test otel_cli
```

The tests cover empty input, trace-id determinism and a real mock run with
agent/chat/tool spans.
