# Dashboard Gate Metrics — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: dashboard gate card grid and CLI wiring.

## Command

```bash
cargo test dashboard --lib
```

## What this proves

- When a gate report is supplied, the dashboard renders a “门禁指标” section
  with runs, completed, crash-free, recovery, replay, tool success, audit
  completeness, denied observations and TTFT p95.
- Percentages format as basis points (`5000 -> 50.00%`).
- An absent gate report renders no gate section; the empty dashboard still
  renders and closes `</html>`.
- Dynamic dashboard text remains escaped; the existing escaping test passes.

## What this does not prove

- That the numbers are a release-gate pass; they are local structural
  projections.
- M19–M23 or model-quality data; use `answer-quality`.
- Real-provider TTFT; only streaming runs have samples.
- That the extra journal read is free; the dashboard reads the directory twice.
