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
5. Running the suite still requires a real provider and a reviewed task set.
   The eval CLI refuses a `model_quality` suite without `--config`; `--validate-only`
   remains the structure-check path. This prevents an accidental deterministic-mock
   run from being mistaken for model-quality evidence.

## Consequences

- The suite is structurally validated in CI and locally without provider
  spend.
- Structural validity is not benchmark evidence. M1/M2/M3 remain unmeasured
  until the suite runs against a real provider and is reviewed.
- The write opt-in is per case and explicit; it does not change default
  permissions for any existing suite.
- Rubric terms are intentionally narrow and may need calibration after the
  first real run.

## Addendum (2026-11-12)

- The first real DeepSeek run exposed three calibration defects: the
  checkpoint expectation defaulted to false for tool tasks, the objective did
  not state the `saturating_sub` rubric term, and required evidence sources
  used exact matching instead of accepting workspace absolute paths. All three
  were fixed before the recorded baseline.
- Three post-calibration runs: 17/24 task runs passed (70.83%), rubric 52/52,
  one hard-gate failure (`checksum-file`, run 2), decision completeness above
  the 8000 bps floor. The repair prompt now restates claim/evidence/conflict
  invariants and the system prompt prefers concrete file paths as evidence
  sources. Human review and calibration are still required.
- See `docs/evidence/real-tasks-benchmark-2026-11-12.md`.
