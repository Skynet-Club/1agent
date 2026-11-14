# ADR 0100: Local Gate Metrics

- Status: accepted
- Date: 2026-11-12
- Related: M1–M23 metric table, SA97-04

## Context

The 0.97 plan requires M4/M5/M6/M13/M17 hard gates and a dashboard that is
derived from journals rather than hand-filled. The existing dashboard covers
usage, latency, job health and alerts, but not recovery, replay completeness,
tool success or audit completeness. There was no single command that answered
"what do the local journals say about the gate metrics?".

## Decision

1. Add `JournalMetricsReport` and `summarize_journal_events` /
   `summarize_journal_dir` in `metrics.rs`.
2. Map journal facts to gate-oriented fields:
   - runs, completed, terminal stops, interrupted;
   - crash-free bps: runs without a missing terminal stop (M14 proxy);
   - resumes, resumed completed, recovery bps (M4 proxy);
   - replay of completed journals, replay bps, compacted replay bps
     (M5/M17, structural replay);
   - tool calls, successful observations, tool success bps (M9);
   - audit completeness via `AgentAnswer::validate` (M13);
   - denied observations (M6 proxy, not a full security incident count);
   - input/output tokens, cost micros and context compactions (M10/M17);
   - run latency p50/p95/max from first/last event timestamps (M8 proxy).
3. Add `1agent metrics --journal-dir DIR [--json]`. The command reads only
   `*.jsonl` files directly under the directory, sorted for determinism.
4. Percentages are basis points with `null` when the denominator is zero, so
   an empty directory cannot report a fake 100%.

## Consequences

- Gate metrics are reproducible offline from local journals and do not
  require a metrics server.
- The numbers are structural. Replay is a journal replay, not a model
  re-run; audit completeness uses the answer validator; denied observations
  are a proxy for M6, not proof of zero security incidents.
- M7 TTFT is not reported by this projection alone; ADR-0105 adds
  `StreamFirstToken` for streaming runs. M14 is reported as a crash-free proxy
  (runs with a terminal stop), not as real session stability. M19–M23 are
  outside this projection and live in ADR-0103.
- M17 has a forced-compaction fixture: a four-tool-step run with
  `auto_compact_tokens = 1` emits `ContextCompacted`, replays and audits
  cleanly, and the metrics report records `compacted_replay_bps = 10000`.
- A corrupt or unreadable journal fails the whole command instead of silently
  dropping a run.
