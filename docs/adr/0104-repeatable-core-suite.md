# ADR 0104: Eight-Case Repeatable Core Suite

- Status: accepted
- Date: 2026-11-12
- Related: SA97-03, M5

## Context

The core contract suite had six cases. SA97-03 asks for eight repeatable
tasks as the 1M/real-world benchmark entry point. The suite is a contract
fixture, not a model-quality suite, so the new cases must be deterministic and
cannot require a network or a real provider.

## Decision

1. Add `read-directory-rejected`: a read target that exists but is not a
   regular file. The tool fails with `tool_failed`, the deterministic provider
   answers `needs_evidence` with a medium-impact unknown, and a checkpoint is
   still written.
2. Add `sleep-then-honest`: the deterministic provider sleeps for a bounded
   interval and then returns the honest `needs_evidence` answer without a tool
   call. This exercises the delay/cancellation-adjacent path.
3. Rename the suite to `core-v3`, version 3, eight cases.
4. Add `tests/eval_repeatability_cli.rs`: run the suite twice, assert 8/8,
   compare a verdict projection (id, pass/fail, stop reason, answer status,
   evidence/claim/unknown/flip counts, audit status). The report
   `content_sha256` is not compared because it includes per-case
   `duration_ms`.

## Consequences

- The default gate now covers eight deterministic cases; `verify.sh` reports
  8/8.
- Repeatability is about verdicts and invariants, not wall-clock timing. Two
  runs can have different content digests while agreeing on every verdict.
- The suite remains a contract fixture and cannot be used as model-quality or
  “real task” evidence. A real eight-task benchmark still needs a configured
  provider and reviewed tasks.
- Historical status documents that say 6/6 are not rewritten; they describe
  earlier suite versions.
