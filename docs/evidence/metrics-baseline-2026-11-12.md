# Local Metrics Baseline (2026-11-12)

Status: **local structural baseline, not a release-gate pass**
Source: `1agent metrics --journal-dir` over local journals after ADR-0101.

## Controlled resume baseline

A deterministic read task was run once, truncated through its first
checkpoint, and resumed into a fresh journal directory.

```text
journals=3 runs=3 completed=2 interrupted=1 resumes=1
recovery=100.00% replay=100.00% compacted_replay=- tool_success=100.00% audit_complete=100.00% denied=0
run latency ms: p50=2 p95=4 max=4 samples=3
```

This is the M4 shape: the interrupted copy lowers completion, the resumed
copy completes, and recovery is 100% for this one sample. It is not a 95%
aggregate over real crashes.

## Real local journal snapshots

`/home/skynet/.journal` (88 journals, mixed historical/real runs):

```text
journals=88 runs=88 completed=60 interrupted=3 resumes=0
recovery=- replay=100.00% compacted_replay=- tool_success=86.17% audit_complete=100.00% denied=5
tokens in=2982513 out=358754 cost_micros=3275 compactions=0
run latency ms: p50=23718 p95=72889 max=123644 samples=88
```

`/home/skynet/1-agent/.journal` (90 journals, heavily test-generated):

```text
journals=90 runs=90 completed=28 interrupted=3 resumes=0
recovery=- replay=100.00% compacted_replay=- tool_success=50.00% audit_complete=100.00% denied=0
tokens in=112399 out=51899 cost_micros=106 compactions=0
run latency ms: p50=1 p95=13635 max=30003 samples=90
```

## Interpretation

- Replay is 100% for completed journals on both snapshots. Replay here is
  structural journal replay, not a model re-run.
- Audit completeness is 100% under `AgentAnswer::validate`.
- Tool success is 86.17% in the historical journals and 50% in the
  test-generated repo journals; the latter is dominated by failure-injection
  tests and is not a product-quality signal.
- Five denied observations were recorded in the historical set; this is an
  M6 proxy, not a security-incident count.
- There are no `ResumeStarted` journals in either snapshot, so aggregate
  recovery is `null`; only the controlled sample above exists.
- Latency is first-to-last journal event time, not TTFT.

## Not covered

M1/M2/M3 (8h benchmark, autonomy hours, interventions), M7 TTFT, M10 real
pricing, M11/M12 memory/skill reuse, M14 crash-free sessions, M18 external
audit, M19–M23 golden-set/blind review. The 2h recovery soak is a separate
duration run.
