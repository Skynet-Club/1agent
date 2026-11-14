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
5. `1agent run --canary-sample PATH` appends one sample per run after the run
   finishes: `success` is true only when the run stops with `Completed`, and
   `latency_ms` is the wall-clock duration. The line contains no task text or
   output. Appending is create-if-missing and never overwrites; a write failure
   prints a warning after the run rather than discarding the run result.
6. The canary command itself does not collect live traffic or persist canary
   state. The caller supplies the sample file for that invocation; each
   invocation recomputes the decision. This is a gate, not an observability
   system.

## Consequences

- The full local lifecycle is reachable from the CLI: capture → register →
  static evaluation → activate → canary → rollback.
- Samples can be fabricated. A canary report only proves what the supplied
  samples imply; it is not a quality claim about live traffic.
- Without a baseline, a rollback decision marks the canary rolled back but
  cannot restore a previous active version.
- Canary state is not persisted between invocations; the sample file is the
  record. `run --canary-sample` makes that file easy to collect from real
  runs, but it still does not journal canary decisions or automatically
  promote.
- `success` means the run stopped `Completed`. A run that stops
  `NeedsEvidence` or `NeedsAuthorization` counts as a failed canary sample,
  which is deliberate but may be strict for exploratory tasks.
- A sample write failure does not fail the run; the warning is the only
  signal.

## Evidence

```bash
cargo test skill_canary --lib
cargo test --test skill_canary_cli
```

The unit tests cover decision thresholds, rollback, latency, missing baseline
and JSONL parsing. The CLI tests cover healthy promotion, failure rollback,
pending samples and malformed input with a line number.
