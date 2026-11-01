# Status v0.63.0

Date: 2026-11-02
Status: retry timing outcomes implemented

## Changes since v0.62.0

- Retry delay emits `provider_retry_elapsed`.
- Each timing records:
  - requested delay;
  - elapsed duration;
  - `completed` or `cancelled`.
- Replay preserves timing outcomes in `retry_timings`.
- Audit rejects timing without a preceding schedule.
- Audit rejects delay mismatch against the current schedule.
- Audit rejects elapsed timing above 2 seconds.
- Cancellation records its timing before the terminal stop.

## New finding codes

```text
provider_retry_outcome_without_schedule
provider_retry_delay_mismatch
provider_retry_elapsed_above_cap
```

## Verification

| Check | Result |
|---|---|
| Unit tests | 106 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| completed zero-delay timing | passed |
| cancelled timing | passed |
| replay timing sequence | passed |
| timing without schedule | rejected |
| delay mismatch | rejected |
| elapsed above cap | rejected |
| normal audit | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Timer precision and runtime scheduling can overshoot requested delay.
- A 2-second elapsed ceiling is diagnostic, not a hard timing guarantee.
- Timing is wall-clock duration from a monotonic clock but stored as milliseconds.
- No retry timing distribution metrics are produced.

## Next requirement

Bind self-evolution experience transitions to supersession and conflict invariants:

1. prevent multiple active versions in the same scope;
2. require superseding experience to exist and match kind/scope;
3. reject expired experience activation;
4. preserve rollback history;
5. test replacement and conflicting activation.
