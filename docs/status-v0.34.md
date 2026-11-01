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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 84e8bd08f59915152fe9cb2f8aa479f68601c47b
git dirty: false

Linux binary:
e048c3e0de8d02192074c2fb922b0e142b22a2e0378c092100f37650bcf90971

Windows binary:
9c85b63a68d9df16d0892387d8bd0c43ef2605f161d46d499a5781d509789c45

Linux tar.gz:
c2e421cfc5ccf1c8d470501b9e66c2f025a52a1f00edbc477884875acec16416

Windows zip:
a7403cfa94258a401a27234abe7a36b851777922128825be8865e5b1d3a1b7d2

Release manifest:
5eabb58fd94ec50a935cde45547c3afaa595346ad1a06e3218d9c8593ccad3c7
```

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
