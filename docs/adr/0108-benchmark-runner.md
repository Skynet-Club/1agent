# ADR 0108: Benchmark Runner

- Status: accepted
- Date: 2026-11-12
- Related: SA100-72, M1/M5

## Context

`1agent benchmark --input` summarized caller-supplied JSONL samples, but
there was no command that produced those samples from an evaluation suite.
The 0.97/1.0 benchmark gate therefore required a manual loop and hand-written
JSONL.

## Decision

1. Add `1agent benchmark-run --suite PATH --runs N [--config PATH]
   [--allow-network] [--timeout-seconds N] [--min-*-bps N] [--json]`.
2. For each run, execute the suite in-process with the same deterministic
   provider or configured providers as `eval`.
3. Map each completed suite report to a `BenchmarkSample`:
   - `completed` = total > 0, all cases passed, zero hard-gate failures;
   - `crashed` = the suite call returned an error (not a process abort);
   - `replay_verified` = report integrity validates and every case audit
     passes (structural, not a model re-run).
4. Summarize with the existing `BenchmarkPolicy` gate and exit non-zero when
   it fails.
5. Bound runs to 1..=100000.

## Consequences

- A suite can be repeated without an external harness, and the result feeds
  the existing gate and JSON consumers.
- A real process crash aborts the runner; it does not become a sample. For
  crash-injection evidence use `recovery_soak` instead.
- `replay_verified` is structural and does not prove model determinism.
- The default deterministic provider plus a contract fixture is not a
  real-task or model-quality benchmark; use a `model_quality` suite and
  configured provider for M1 claims.
- Runs share the same provider objects; a stateful provider may change
  behavior between iterations.
