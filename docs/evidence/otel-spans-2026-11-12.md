# OTel GenAI Span Projection — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `otel::spans_from_events` and `1agent otel --journal PATH [--json]`.
This is a JSON projection, not an OTel SDK/exporter.

## Commands

```bash
cargo test otel --lib
cargo test --test otel_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 2 tests
test otel::tests::empty_events_emit_no_spans ... ok
test otel::tests::trace_id_is_stable_for_a_run ... ok

test result: ok. 2 passed; 0 failed

running 1 test (tests/otel_cli.rs)
test otel_cli_projects_agent_chat_and_tool_spans ... ok

test result: ok. 1 passed; 0 failed
```

## What this proves

- Empty journals produce no spans.
- Trace ID derivation from the run id is stable.
- A real mock run produces an `invoke_agent` root, a `chat` span with
  `gen_ai.provider.name=deterministic`, and an `execute_tool read_file` span
  with `gen_ai.tool.name=read_file`.
- Span IDs and trace IDs are deterministic for replay.
- Run status maps completed to `ok`, needs-input to `unset`, and failures to
  `error`.

## What this does not prove

- That a live collector accepts the spans; no OTel exporter/propagation is
  implemented.
- Full OTel GenAI semantic-convention coverage; this is a subset.
- Cross-process trace context; spans are projected within one journal.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/otel.rs`
- `src/main.rs` (`otel`)
- `tests/otel_cli.rs`
- `docs/adr/0056-otel-genai-spans.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
