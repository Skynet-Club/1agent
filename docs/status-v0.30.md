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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 29868df42e56ec6ba5ce247dd42fbbd7d3e8a1d0
git dirty: false

Linux binary:
bc36750938d6bee5dea4f0cf5b7d66a0ba6ce2bab76370ed7e5e730b517cb7fc

Windows binary:
140590c26eb27ff034e7700db36892f932a62e9e8cad14368616958c561d2ff0

Linux tar.gz:
30dbf7d2d80333a9e99851d2b0dc006b7dd44672a20c15bad4103109f25d06c2

Windows zip:
cb040bdf213806bce1d82210b8a5ed42106184093fe218c0b5199861d14995ae

Release manifest:
0aa0c5eb352005ed91b801b5aa3958f7cc3faf8df900cb9e4f368b10d872d977
```

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
