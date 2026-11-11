# OTLP/HTTP JSON Export — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `otel_export::otlp_json`, `otel_export::export_otlp`,
`1agent otel-export`.

## Commands

```bash
cargo test otel --lib
cargo test --test otel_export_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 2 tests (tests/otel_export_cli.rs)
test otel_export_posts_an_otlp_json_envelope ... ok
test otel_export_reads_the_bearer_token_only_from_the_environment ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- A real mock-provider journal produces spans that are wrapped in one
  `resourceSpans` with `service.name` and one `scopeSpans` named `1agent`.
- Root spans keep 32-hex trace ids, 16-hex span ids, OTLP kind 1 (internal),
  string-valued nanosecond timestamps and `stringValue` attributes.
- A loopback collector receives the POST with HTTP 200; the response status is
  reported as JSON.
- No `Authorization` header is sent unless `--token-env` names a set variable;
  the named variable's value becomes a bearer token and is not printed.
- A missing token variable and an insecure remote HTTP endpoint fail closed
  before any request.

## What this does not prove

- Interoperability with a real OTLP collector or commercial APM; only a
  loopback mock was used.
- gRPC OTLP, protobuf wire format, batching, retry queues, sampling or TLS
  termination.
- Numeric attribute typing: every attribute is exported as `stringValue`.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/otel_export.rs`
- `src/main.rs` (`otel-export`)
- `tests/otel_export_cli.rs`
- `docs/adr/0082-otlp-http-export.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
