# Local Gate Metrics — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: journal-derived gate metrics and the `1agent metrics` command.

## Commands

```bash
cargo test journal_metrics --lib
cargo test metrics --lib
cargo test --test metrics_cli
cargo test compacted_run_replays_and_audits --lib
```

## Real sample

```text
journals=1 runs=1 completed=1 interrupted=0 resumes=0
recovery=- replay=100.00% compacted_replay=- tool_success=100.00% audit_complete=100.00% denied=0
tokens in=0 out=0 cost_micros=0 compactions=0
run latency ms: p50=7 p95=7 max=7 samples=1
```

The sample is a deterministic mock-provider run, so token/cost are zero by
construction; a configured provider reports its usage in `ModelResponded`.

## What this proves

- Completed runs, terminal stops and interrupted journals are counted
  separately.
- Resumed journals that complete are counted for the M4 proxy; a resumed
  journal that does not complete lowers recovery bps.
- Completed journals are structurally replayed; compacted runs get their own
  replay rate (M5/M17 proxies).
- Tool success counts only `ToolStatus::Ok` observations (M9).
- Audit completeness uses `AgentAnswer::validate` (M13 proxy).
- Denied observations are counted (M6 proxy).
- Tokens/cost come from `ModelResponded`, and run latency comes from the
  first/last event timestamps (M8 proxy).
- Empty inputs return zeros with `null` percentages, not a fake 100%.
- A corrupt journal fails the command; it is not silently skipped.
- The CLI prints the projection as JSON or a compact human summary.
- M14 is reported as `crash_free_bps`: runs with a terminal stop divided by
  all runs. It is a crash-free proxy, not a measure of user-perceived session
  stability.
- M17 has a forced-compaction fixture: a four-tool-step run with
  `auto_compact_tokens = 1` emits `ContextCompacted`, replays, audits and
  reports `compacted_replay_bps = 10000`.

## What this does not prove

- M1/M2/M3 (8h benchmark, autonomy hours, interventions) and M7 TTFT (no
  first-stream-chunk timestamp in the journal).
- M10 cost without configured provider pricing; the project does not embed
  prices.
- M11/M12 memory/skill reuse rates and M14 crash-free sessions.
- M19–M23 golden-set/blind-review metrics.
- That replay reproduces a model run; it replays the journal.
- That zero denied observations means zero security incidents; it is one
  local signal.
