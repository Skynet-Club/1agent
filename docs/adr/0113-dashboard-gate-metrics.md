# ADR 0113: Dashboard Gate Metrics

- Status: accepted
- Date: 2026-11-12
- Related: SA97-04

## Context

`1agent dashboard` rendered usage, latency, job health and alerts, but not
the gate metrics introduced by `1agent metrics`. An operator had to open two
reports to see whether recovery, replay or audit completeness were healthy.

## Decision

1. `DashboardInputs` gains an optional `gate: Option<&JournalMetricsReport>`.
2. When present, the dashboard renders a “门禁指标” card grid: runs,
   completed, crash-free, recovery, replay, tool success, audit completeness,
   denied observations and TTFT p95 when sampled.
3. `1agent dashboard` computes the gate report from the journal directory
   using the same `summarize_journal_dir` as `1agent metrics`. A missing
   directory renders no gate section; a corrupt journal fails the command.
4. The dashboard remains a static, escaped HTML report with no scripts.

## Consequences

- One report shows operational usage and the local gate picture.
- The journal directory is read twice (usage events and per-journal metrics);
  this is acceptable for a local report but not optimized.
- M19–M23 are not on this dashboard; use `answer-quality`. M1–M3 and real
  TTFT still depend on benchmark data.
- The gate section is a projection of local structural metrics, not a release
  gate pass.
