# ADR 0086: Benchmark Gate Metrics

- Status: accepted
- Date: 2026-11-12

## Context

The 1.0 exit criteria require an 8-hour benchmark with at least 70% task
completion, at least 99.5% crash-free operation and 100% replay. There is no
long-running benchmark runner yet, and inventing a headline number from a
short local test would be dishonest. The metrics still need one deterministic
definition so a future runner, CI job or external operator computes them the
same way.

## Decision

1. Add `benchmark::BenchmarkSample` with three independent booleans:
   `completed`, `crashed`, `replay_verified`. A sample marked `crashed` is
   never counted as completed or replay-verified.
2. `benchmark::summarize(samples, policy)` computes:
   - completion bps = completed and not crashed / iterations;
   - crash-free bps = not crashed / iterations;
   - replay bps = replay-verified and not crashed / iterations;
   - `passed` only when all three reach the policy thresholds.
   The default policy is 7000 / 9950 / 10000 bps, matching the plan targets.
3. Empty input, more than 1,000,000 samples or a threshold above 10,000 fails
   closed. Bps uses integer division; there is no rounding up.
4. `benchmark::parse_samples` accepts one JSON object per non-empty line and
   reports the source line on malformed input. Unknown fields are rejected.
5. This module is a gate and a calculator, not a runner. It does not execute
   tasks, measure latency, prove task quality or create samples.

## Consequences

- A benchmark report is reproducible from the same sample stream.
- Crashed samples cannot inflate completion or replay rates.
- The plan's 8-hour run, sample collection and task-quality evaluation remain
  external work. A passing report only means the supplied samples met the
  numeric thresholds.
- The defaults are targets, not evidence; a caller must override them for a
  different policy.

## Evidence

```bash
cargo test benchmark --lib
```

The tests cover a clean window, a crashed sample being excluded from
completion/replay, independent threshold failures, JSONL parsing with line
numbers, and empty/oversized/invalid input.
