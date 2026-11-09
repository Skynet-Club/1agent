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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Backoff is deterministic and has no jitter.
- Delay is per immediate fallback sequence, not provider-global.
- No circuit breaker or cross-run provider health exists.
- Remaining-time boundaries are validated by code paths but not real slow-network tests.

## Verified clean release

```text
source commit: 230726300827d0ffcb769656b105caa633f0e5e6
git dirty: false

Linux binary:
fccf1f6a4a3e19d7addfffe49358700603de3f235b0190cc2d6a7644906b2445

Windows binary:
356bb70eaca3823768b2c1b0ec13c25b67fbb5ba4fd64a3fa90409a80816a4aa

Linux tar.gz:
cdcd1f198264eeae3e63839641e930a3749859577c4c6e5f9a768240fc9a42b9

Windows zip:
f34d0f99946605c1bd45889a23720b2c2aff4902ce25c484aa8fc27d049d8a17

Release manifest:
8a4835bd7b103cefb43d8e67bb522ed95ad16351d7e644918404397ff625c912
```

## Next requirement

Bind retry scheduling to audit invariants:

1. a scheduled retry must name a route in the current plan;
2. the next selected route must match the scheduled target;
3. stalled retries without a following route must be visible;
4. delay must not exceed the documented cap;
5. preserve unscheduled direct fallback compatibility.
