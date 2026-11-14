# ADR 0082: OTLP/HTTP JSON Export

- Status: accepted
- Date: 2026-11-12

## Context

SA99-64 asks for OTel GenAI export so enterprise APMs can observe agent runs.
The existing `otel` command projects the journal into deterministic spans, but
it only prints JSON; it cannot reach a collector.

## Decision

1. Add `otel_export::otlp_json(spans, service_name)`:
   - one `resourceSpans` entry with `service.name`;
   - one `scopeSpans` entry named `1agent` with the crate version;
   - every span keeps its 32-hex trace id, 16-hex span id, optional parent,
     name, kind (internal=1, client=3), start/end Unix nanoseconds as strings,
     and attributes encoded as OTLP `stringValue`;
   - OK/ERROR status maps to OTLP code 1/2; unset omits `status`.
2. Add `otel_export::export_otlp(endpoint, token_env, payload)`:
   - endpoint must be HTTPS or loopback HTTP, without embedded credentials;
   - redirects are not followed; timeout is 5 seconds;
   - a bearer token is read only from the named environment variable and is
     never logged or returned;
   - non-2xx responses are errors that include only the status code.
3. Add `1agent otel-export --journal PATH --endpoint URL [--service-name NAME]
   [--token-env VAR] [--json]`. An empty journal is an error rather than an
   empty export. Output reports the span count and HTTP status; the endpoint
   and token are not echoed.
4. This is an OTLP/HTTP JSON adapter, not an OTel SDK, gRPC exporter, batch
   processor, retry queue or sampling layer.

## Consequences

- A collector accepting OTLP/HTTP JSON can receive the same spans `otel`
  prints, with standard GenAI attribute names.
- The host supplies the full traces URL (for example `/v1/traces`) and owns
  TLS termination and collector availability.
- Failed exports return an error; the caller decides whether to retry. There is
  no background buffer, so a run journal remains the source of truth.
- Attribute values are exported as strings, which OTLP permits but some
  backends may index as text.

## Evidence

```bash
cargo test otel --lib
cargo test --test otel_export_cli
```

The exporter test creates a real mock-provider journal, posts to a loopback
collector, and checks the resource, scope, span ids, kind and attribute shape,
plus environment-only bearer auth, a missing token and an insecure endpoint.
