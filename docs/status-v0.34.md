# Status v0.34.0

Date: 2026-11-02
Status: checkpoint-bound route policy equivalence implemented

## Changes since v0.33.0

- Every new checkpoint stores:
  - SHA-256 of the canonical route plan plus policy sources;
  - effective route policy sources.
- Resume recomputes the plan from the restored task and current provider manifests.
- Resume is rejected when the recomputed route digest differs from the checkpoint.
- This catches changes to:
  - route objective;
  - provider p95/cost budgets;
  - hard filters;
  - provider manifests or ordering;
  - policy source attribution.
- Legacy checkpoints without route metadata remain readable.

## Verification

| Check | Result |
|---|---|
| Checkpoint route digest persistence | passed |
| Checkpoint policy-source persistence | passed |
| Equivalent-policy resume | passed |
| Changed-objective resume rejection | passed |
| Legacy checkpoint decode | implemented through serde defaults |
| Unit tests | 76 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- The route digest includes a provider snapshot indirectly through the canonical plan; provider
  configuration changes that do not alter plan output are not rejected.
- Resume uses the caller's current runtime objective and then verifies equivalence; it does not
  automatically choose the stored objective.
- Legacy checkpoints cannot be retroactively proven route-equivalent.

## Next requirement

Make resume policy recovery ergonomic and explicit:

1. Expose checkpoint route metadata in resume status.
2. Load the stored effective objective when no override is supplied.
3. Reject explicit overrides that conflict with the checkpoint.
4. Report whether a checkpoint is route-aware or legacy.
5. Add CLI-level resume tests.
