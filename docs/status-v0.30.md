# Status v0.30.0

Date: 2026-11-02
Status: explicit route objectives implemented

## Changes since v0.29.0

- Added four route objectives:
  - `quality` (default): quality, then cost, latency and stable identity;
  - `latency`: p95 latency, then quality, cost and stable identity;
  - `cost`: combined token price, then quality, latency and stable identity;
  - `balanced`: ordinal quality/cost/latency ranks with deterministic tie-breaking.
- Privacy, network permission, risk, mode, tools and structured-output requirements remain hard
  filters and run before objective ranking.
- Added `--route-objective` to `run` and `resume`.
- Route-selection reasons now include the selected objective and are persisted in the journal.
- Missing or unsupported objectives are rejected at CLI parsing.

## Verification

| Check | Result |
|---|---|
| Quality route | expected provider |
| Balanced route | expected provider |
| Latency route | expected provider |
| Cost route | expected provider |
| Remote filtered when network disabled | retained |
| Tool capability filtered | retained |
| Unit tests | 73 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- Quality and latency/cost values are operator-supplied estimates, not live measured values.
- Balanced uses ordinal ranks rather than calibrated utility weights.
- Route decisions are static for a run and do not adapt from per-attempt latency.
- Learned experience cannot participate in routing yet.

## Next requirement

Make routing metadata inspectable before execution:

1. Add a route-plan command that shows ranked candidates and reasons.
2. Include credential readiness and hard-filter rejection causes.
3. Never perform network calls in route planning.
4. Add machine-readable output.
5. Keep route planning deterministic.
