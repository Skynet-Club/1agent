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
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Audit validates structure and references, not remote latency truth.
- Delay timestamps are not recorded, only requested milliseconds.
- A cancelled schedule does not record elapsed sleep time.
- No jitter or circuit breaker exists.

## Next requirement

Record actual retry timing and cancellation outcomes:

1. add scheduled and completed timestamps or elapsed duration;
2. distinguish elapsed completion from cancellation;
3. bound timing metadata;
4. replay timing outcomes;
5. test zero, completed and cancelled delays.
