# Status v0.61.0

Date: 2026-11-02
Status: bounded retry backoff implemented

## Changes since v0.60.0

- `RunBudget` adds `provider_retry_backoff_ms`, default `50`.
- CLI run adds `--provider-retry-backoff-ms`.
- Retry delay follows `base * 2^(retry-1)` with a 1-second hard ceiling.
- A base of zero disables waiting while retaining scheduling evidence.
- Remaining run time is checked before a delay is accepted.
- A delay that consumes the remaining budget terminates with a needs-evidence answer.
- `provider_retry_scheduled` records delay, next provider and model.
- Replay preserves delay sequence in `retry_delays_ms`.
- Backoff sleep is cancellation-responsive.

## Verification

| Check | Result |
|---|---|
| Unit tests | 104 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| zero-delay behavior | passed |
| exponential growth | passed |
| 1-second cap | passed |
| journaled retry schedule | passed |
| replay delay sequence | passed |
| cancellation during delay | passed |
| fallback after delay | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Backoff is deterministic and has no jitter.
- Delay is per immediate fallback sequence, not provider-global.
- No circuit breaker or cross-run provider health exists.
- Remaining-time boundaries are validated by code paths but not real slow-network tests.

## Next requirement

Bind retry scheduling to audit invariants:

1. a scheduled retry must name a route in the current plan;
2. the next selected route must match the scheduled target;
3. stalled retries without a following route must be visible;
4. delay must not exceed the documented cap;
5. preserve unscheduled direct fallback compatibility.
