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
| Clean-source package gate | pending |

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
