# First-Token Instrumentation — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: `StreamFirstToken`, runtime instrumentation and TTFT metrics.

## Commands

```bash
cargo test stream_sink_receives_deltas --lib
cargo test journal_metrics --lib
cargo test --test metrics_cli
```

## What this proves

- A streaming attempt journals a `StreamFirstToken` event with the
  provider/model and a non-negative elapsed value.
- The journal metrics projection turns those events into a p50/p95/max/count
  TTFT digest.
- A journal without streaming events has `ttft_latency: null`, not zero.

## What this does not prove

- Real-provider TTFT; the tests use a deterministic streaming provider.
- TTFT for non-streaming runs; they emit no event.
- A per-run “time to first answer” when retries occur; every attempt's first
  token is a separate sample.
- That the first fragment is semantically the first answer token; it is the
  first non-empty content fragment.
