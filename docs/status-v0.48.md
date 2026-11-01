# Status v0.48.0

Date: 2026-11-02
Status: impact-ranked unknowns implemented

## Changes since v0.47.0

- Added `ImpactLevel`: low, medium, high; default medium.
- Unknown descriptions must be non-empty.
- High-impact unknowns must include a non-empty reduction method.
- Evaluation expectations add `min_high_impact_unknowns`.
- JSON scorecards report low, medium and high unknown counts.
- Authorization, missing-reasoning and conflict examples now explicitly mark high impact.

## Verification

| Check | Result |
|---|---|
| High impact with reduction | passed |
| High impact without reduction | rejected |
| Impact counts | passed |
| Core evaluation | 6/6 passed |
| Unit tests | 85 passed |

## Known limitations

- Impact is model/author supplied, not calibrated.
- Assumptions remain free-text strings in this iteration.
- Impact ranking is not yet used by runtime stop logic.

## Next requirement

Version assumption records:

1. Introduce structured assumptions with impact.
2. Require high-impact assumptions to have verification methods.
3. Preserve legacy string decoding.
4. Report assumption counts by impact.
5. Keep answer schema backward compatible for one release.
