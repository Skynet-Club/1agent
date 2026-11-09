# Evaluation Standard

Date: 2026-11-02

Status: offline contract evaluation implemented; real-model evidence pending

## Purpose

Evaluation must distinguish plumbing correctness, decision completeness, task correctness,
repeatability, safety, cost and human agreement. A single aggregate score cannot represent these
properties.

## Suite kinds

### Contract fixture

Uses deterministic scripted providers and proves protocol, safety and semantic invariants. It may
validate complete tool call/observation/answer sequences.

It cannot be promoted as evidence that a model reasons well.

### Model quality

Requires:

- a ready execution status;
- declared provider requirements;
- a named reviewer;
- calibration required;
- held-out case identity;
- provider/model/version recording.

Contract fixtures are rejected for experience promotion and model-quality claims.

## Data splits

Use disjoint:

```text
development
tuning
held-out
```

Rules:

- held-out cases are not used for prompt tuning or routing weights;
- case IDs and suite content are versioned;
- subjective tasks require independent reviewer labels;
- freshness-sensitive evidence includes observation/expiry metadata;
- prompt-injection content is labeled as untrusted data;
- provider failures and invalid outputs remain visible rather than being imputed.

## Per-case result

An evaluation case records:

- expected stop reason and answer status;
- evidence, claim, unknown, flip and conflict counts;
- hard-gate status;
- rubric checks;
- low/medium/high impact unknown and assumption counts;
- stale evidence count;
- decision-completeness rollup;
- provider/model, duration and usage;
- audit status and issues.

## Decision completeness

A case is complete when:

- a conditional answer has a target-bound flip condition;
- every flip condition is bound;
- high-impact unknowns have reduction methods;
- a completed answer does not depend on a high-impact assumption.

The suite aggregate reports:

- cases and complete cases;
- completion rate in basis points;
- conditional and bound-flip counts;
- unbound flips;
- high-impact unknowns without reduction;
- completed answers with high-impact assumptions.

The floor defaults to `10000` bps for model-quality suites and `0` elsewhere. It can be configured
within `0..=10000`; the report records policy version and whether the floor was defaulted or
explicit.

Decision completeness is necessary but not sufficient. A complete answer can still be wrong.

## Core metrics

| Metric | Question |
|---|---|
| pass rate | How often did a case satisfy all gates? |
| pass^k | How often did a task succeed across all k repetitions? |
| hard-gate failures | Did audit, safety or decision completeness fail? |
| unsupported certainty | Did a completed answer lack claims/evidence? |
| evidence provenance failures | Can evidence be tied to declared claims? |
| token cost | What usage was reported by the provider? |
| monetary cost | What normalized cost was reported or configured? |
| p50/p95 latency | What latency is typical and tail-heavy? |
| retry/fallback rate | How often was the primary route replaced? |
| reviewer agreement | Can humans apply the rubric consistently? |
| calibration error | Are numeric confidence claims reliable? |

## pass^k

For `n` tasks and `k` repetitions, report both:

- mean single-run success;
- the probability that all `k` runs of a sampled task succeed.

The repetition count and sampling rule must be recorded. A high mean pass rate with poor pass^k
indicates unstable behavior.

## Gates

Minimum hard gates:

- audit passes;
- answer schema is supported;
- evidence/provenance invariants pass;
- action receipts respect permissions;
- a model-quality report meets its completeness floor;
- all aggregate fields reconstruct from case results.

Recommended evidence stages:

| Stage | Purpose | Required claim |
|---|---|---|
| Fixture | Protocol regression | “Contracts pass under scripted providers.” |
| Adapter smoke | Endpoint compatibility | “A configured provider completed/failed normally.” |
| Provider pilot | Distributional quality | “Provider/model X met recorded gates on corpus Y.” |
| Held-out | Generalization | “The result did not use tuning examples.” |
| Production candidate | Operational envelope | “Quality, stability, cost and latency meet approved thresholds.” |

## Report trust

Reports include a canonical content digest. `eval-inspect` recomputes arithmetic and aggregates,
checks policy/version compatibility and verifies the digest.

Optional authenticated reports use an `evaluation_report` approval envelope and key registry.
Authenticity is separate from content binding and evaluation truth.

## Evaluation workflow

1. Define the task family and expected outcome.
2. Write rubric and hard gates.
3. Review ambiguous cases.
4. Freeze suite version and held-out split.
5. Run with fixed provider/model/parameters.
6. Recompute audit and aggregate invariants.
7. Repeat for stability.
8. Review failures against raw observations.
9. Record cost, latency and unsupported certainty.
10. Promote only the claims supported by the evidence stage.

## Invalid claims

Do not infer:

- real-model quality from deterministic fixtures;
- correctness from decision completeness;
- provider superiority from one run or one task;
- safety from prompt instructions alone;
- calibrated confidence from model self-report;
- production readiness from a successful build.
