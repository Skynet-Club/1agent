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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 6300e4dcb517354fcfe4d9ce66903f853c3bd2fc
git dirty: false

Linux binary:
98dfd829c2384388eab5113b01dce05370d4654b8052562084bde4d83355ac80

Windows binary:
32a99943d8a20e1e66a6396acc54a3184aa0908d0804f1363fa5ef5df19c8825

Linux tar.gz:
1eba09372050f6e51f129378e42d6043b0983f6225616a4ca53b6f10243b9138

Windows zip:
98849ab5bf1754d5dc1b662b101215b57d5c989a9c66995b3e775ffc467e4223

Release manifest:
ae31cd67291b5797294f1a6484541e3a47b41359d4716923b93dbaa7cb78be9d
```

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
