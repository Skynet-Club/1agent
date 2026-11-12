# ADR 0121: Real-Task Suite and Validate-Only Eval

- Status: accepted
- Date: 2026-11-12
- Related: SA97-03

## Context

The 0.97 release requires at least eight real tasks (bug fix, config change,
data processing, research with evidence, long task) with a fixed provider,
temperature and budget. The existing eval harness was read-only and had no way
to validate a model-quality suite without running it. A real suite that cannot
be checked structurally would fail late, after provider spend.

## Decision

1. `EvalCase.allow_write` (default `false`) lets a real task opt into write
   permissions. `run_case` applies it to the task request. Existing contract
   fixtures stay read-only.
2. `eval::validate_suite` checks non-empty cases, unique ids, non-empty
   objectives and rubric terms, and model-quality metadata without running a
   model.
3. `1agent eval --validate-only` exposes the check and reports the suite name,
   kind, status and case count.
4. `evals/real-tasks-v1.json` declares eight real tasks: off-by-one bug fix,
   config change, JSONL aggregation, two-file research comparison, three-step
   planning, invalid-JSON handling, checksum write, and directory summary.
   Five tasks opt into writes. The suite is `model_quality`, `ready`, has
   provider requirements, a required reviewer and calibration.
5. Running the suite still requires a real provider and a reviewed task set;
   the deterministic mock is explicitly forbidden by the provider requirement.

## Consequences

- The suite is structurally validated in CI and locally without provider
  spend.
- Structural validity is not benchmark evidence. M1/M2/M3 remain unmeasured
  until the suite runs against a real provider and is reviewed.
- The write opt-in is per case and explicit; it does not change default
  permissions for any existing suite.
- Rubric terms are intentionally narrow and may need calibration after the
  first real run.
