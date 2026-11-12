# ADR 0088: Skill Canary CLI Gate

- Status: accepted
- Date: 2026-11-12

## Context

`SkillCanary` already models runtime canary promotion and rollback: a minimum
sample count, a failure budget, an optional p95 latency ceiling and a decision
applied through `SkillService`. The CLI had no way to feed it samples, so the
canary step of the skill lifecycle was only reachable from tests and library
callers.

## Decision

1. Add `CanarySample` with `success` and `latency_ms`. `parse_canary_samples`
   accepts one JSON object per non-empty line, rejects unknown fields and
   reports the source line on malformed input.
2. Bound input at 100,000 samples and 16 MiB. More than either fails closed.
3. Add `1agent skill canary --store PATH --name NAME --version VERSION
   --samples PATH [--min-samples N] [--max-failure-bps N]
   [--max-p95-latency-ms N] [--apply] [--json]`. Defaults are 20 samples and
   500 failure bps.
4. The command starts a canary only for a registered version, records every
   sample, computes the decision and prints `pending`, `promote` or `rollback`
   with the sample count and state. With `--apply`, a promote activates the
   candidate; a rollback restores the baseline when one exists. A pending
   decision is a no-op and leaves the active version unchanged.
5. The command does not collect live traffic or persist canary state. The
   caller supplies the sample file for that invocation; each invocation
   recomputes the decision. This is a gate, not an observability system.

## Consequences

- The full local lifecycle is reachable from the CLI: capture → register →
  static evaluation → activate → canary → rollback.
- Samples can be fabricated. A canary report only proves what the supplied
  samples imply; it is not a quality claim about live traffic.
- Without a baseline, a rollback decision marks the canary rolled back but
  cannot restore a previous active version.
- Canary state is not persisted between invocations; the sample file is the
  record. A future live canary should journal samples and state.

## Evidence

```bash
cargo test skill_canary --lib
cargo test --test skill_canary_cli
```

The unit tests cover decision thresholds, rollback, latency, missing baseline
and JSONL parsing. The CLI tests cover healthy promotion, failure rollback,
pending samples and malformed input with a line number.
