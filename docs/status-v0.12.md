# Status v0.12.0

Date: 2026-11-02
Status: opt-in calibration analysis

## Added since v0.11.0

- `annotate calibrate`.
- Reliability gate:
  - configured minimum kappa;
  - configured minimum number of multi-annotated cases;
  - sufficient-data flag.
- Human consensus mapping:
  - supported consensus becomes success;
  - unsupported consensus becomes failure;
  - ties are excluded.
- Calibration metrics:
  - 10-bin reliability table;
  - expected calibration error;
  - Brier score;
  - selective-risk coverage/risk at 0.5 through 0.9 confidence.
- Numeric confidence remains unavailable from the Agent output contract.

## Verification

| Check | Result |
|---|---|
| Unit tests | 51 passed |
| Calibration with reliable labels | passed |
| Calibration refusal without reliable labels | passed |
| ECE/Brier report | passed |
| Core evaluation rubric | 4/4 passed |
| Clippy `-D warnings` | passed |

Release artifacts:

```text
Linux binary:   9461fa2143f8cfd7f6f696d2fac73ddeba68200cfa9e9f213105c47ea55675d2
Windows binary: f96fab286418362708b6fff7197622e1aaa26ead9e078899cd21da5f613e4efc
Linux tar.gz:   efef973676b78464866217f4826310d5aa4de1fe70c7a5c9c10d61119669526e
Windows zip:    71b7336e4f6513032af70ba96ad99e17d4eb4cc4a683054d78264e2076df3ec1
```

## Requirement comparison

| Requirement | Status |
|---|---|
| Reliability threshold before calibration | implemented |
| ECE and Brier | implemented |
| Selective-risk curve | implemented |
| Numeric confidence exposure | intentionally disabled |
| Real reviewer cohort | not available in environment |
| Real-provider evaluation | blocked by missing credentials |

## Known risks

- Human labels are local and unauthenticated.
- The minimum kappa/sample thresholds are configurable and not yet calibrated for production.
- Calibration confidence is only as valid as the reviewed case distribution.
- The Agent still must not emit numeric confidence based on this tool alone.

## Next requirement

Add a redacted gold-set manifest and reviewer schedule:

1. Define case ownership and annotation batches.
2. Record label-set digest and reviewer cohort.
3. Support reviewed-case exclusion from model prompt context.
4. Produce calibration artifacts only from frozen gold-set versions.
5. Preserve opt-in behavior for any numeric confidence experiment.
