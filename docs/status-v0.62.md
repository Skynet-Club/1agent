# Status v0.62.0

Date: 2026-11-02
Status: retry scheduling audit invariants implemented

## Changes since v0.61.0

- The 1-second backoff ceiling is shared from the domain contract.
- A scheduled retry target must exist in the active route plan.
- The next RouteSelected must match the scheduled provider/model.
- Delay above the ceiling is an audit failure.
- Duplicate schedules and schedules without a following route are visible.
- Cancellation and budget exhaustion may legitimately abandon a scheduled retry.

## New finding codes

```text
provider_retry_outside_route_plan
provider_retry_target_mismatch
provider_retry_without_next_route
provider_retry_delay_above_cap
duplicate_provider_retry_schedule
```

## Verification

| Check | Result |
|---|---|
| Unit tests | 105 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| normal scheduled fallback | passed |
| zero-delay schedule | passed |
| over-cap delay | rejected |
| target outside route plan | rejected |
| target mismatch | rejected |
| duplicate schedule | rejected |
| stalled schedule | rejected |
| cancelled schedule termination | accepted |
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Audit validates structure and references, not remote latency truth.
- Delay timestamps are not recorded, only requested milliseconds.
- A cancelled schedule does not record elapsed sleep time.
- No jitter or circuit breaker exists.

## Verified clean release

```text
source commit: fbf5ba1c577b4febb582aaa486efa9d2181636d3
git dirty: false

Linux binary:
1aec8339bb46cc4945f1c5ce1d63e75d7e813ea143795ec297dde9cade153e2a

Windows binary:
2a13c25b9969ae6986e385e2c282acb3fc816e6647ca7eaf5b22545a9ac99f2a

Linux tar.gz:
aa71238b856f794a92eceaa53381ddb80b60d9906bcf1ee4c5fce9ef60fdf838

Windows zip:
4fcdb716dee5f0d198e3ddd0c58a3275aa2a0be2271cd14e3ae574610f79f9ff

Release manifest:
0f37ddf6c7492eeb814d50e3ef2c8644a7c43066cf38c3d213d2ed40223e9c2f
```

## Next requirement

Record actual retry timing and cancellation outcomes:

1. add scheduled and completed timestamps or elapsed duration;
2. distinguish elapsed completion from cancellation;
3. bound timing metadata;
4. replay timing outcomes;
5. test zero, completed and cancelled delays.
