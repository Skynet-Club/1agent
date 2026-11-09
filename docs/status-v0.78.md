# Status v0.78.0

Date: 2026-11-02
Status: integrity-bound evaluation report inspection implemented

## Validation

`eval-inspect` now accepts a report only when all of these invariants hold:

- `total == cases.len()`;
- `passed` and `failed` match case outcomes and sum to `total`;
- `hard_gate_failures` matches case records;
- rubric checks and passes match case rollups;
- decision-completeness aggregates match case rollups exactly;
- the floor status is the comparison of completion rate and configured floor;
- policy version/source metadata is supported and consistent.

Validation failures exit non-zero and identify the violated invariant.

## Verification

| Check | Result |
|---|---|
| Unit tests | 123 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| valid report accepted | passed |
| case total mismatch | rejected |
| aggregate mismatch | rejected |
| floor contradiction | rejected |
| policy validation preserved | passed |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Release artifacts

Source commit: `f56bcd0aec73aa2365f21c545844c9b155d09aae`

Clean-source artifacts were rebuilt after the feature commit. SHA-256:

| Artifact | SHA-256 |
|---|---|
| Linux `tar.gz` | `bfad9bdead4d6ffc51d5c30ccd309968c93eb6f71961ef60fe6aa158551e09d9` |
| Windows `zip` | `ef253245370da39825d00aa529eb648a626a5eb381c28ff2952af55d756c5a4e` |
| Release manifest | `075258fad5df65bfa3eec1dd013d5e3377508a3b4cafa3c8bed3ef8a77d4b66f` |
| Linux binary | `7635f9ea98d4f98e66f90a915129534817a6d20ca1c261fce5986551d8407f36` |
| Windows binary | `97947483c037e7bd0294e45fa03078afdc191a2b980c03a8af32593502da16d9` |

## Known limitations

- Integrity checks validate report arithmetic, not external truth.
- Case results are not recomputed from model or journal evidence.
- Report metadata remains unsigned.
- Legacy reports without policy metadata are normalized, not cryptographically upgraded.

## Next requirement

Expose a compact report-inspection summary for hosts that need status without consuming or
persisting the full case payload:

1. include suite, outcome counts, policy provenance and floor status;
2. keep full normalized JSON available explicitly;
3. preserve current integrity validation;
4. test summary stability and sensitive case-content exclusion.
