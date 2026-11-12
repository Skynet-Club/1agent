# Journal-Derived Stage Latencies — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: route/context/model/tool latency projection from journal timestamps.

## Commands

```bash
cargo test --offline metrics --lib
cargo test --offline --test metrics_cli
cargo test --offline dashboard --lib
```

## What this proves

- A synthetic journal with timestamps `RunStarted=0`, `RouteSelected=10`,
  `ModelRequested=30`, `ModelResponded=80`, `ToolCalled=90`,
  `ToolObserved=110`, `ModelRequested=120`, `ModelResponded=150` projects:
  route 10 ms, context 20/10 ms, model 50/30 ms, tool 20 ms.
- Percentiles use the existing nearest-rank `LatencyDigest`.
- A real local run's journal produces all four digests in both JSON and text
  output (`route latency ms:`, `context latency ms:`, `model latency ms:`,
  `tool latency ms:`).
- Missing boundaries and non-monotonic timestamps are skipped, not zero-filled.
- The dashboard gate test still compiles with the four new report fields.

## What this does not prove

- Journal-write and TUI-render durations; they are not inferred from event
  gaps and remain covered by the write path and TUI frame probe.
- Millisecond precision or provider-internal latency.
- Real-provider benchmark behavior; the CLI fixture uses the deterministic
  local provider.
- That a stage gap is pure work: scheduling and I/O wait are included.
