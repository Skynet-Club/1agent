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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- Timer precision and runtime scheduling can overshoot requested delay.
- A 2-second elapsed ceiling is diagnostic, not a hard timing guarantee.
- Timing is wall-clock duration from a monotonic clock but stored as milliseconds.
- No retry timing distribution metrics are produced.

## Verified clean release

```text
source commit: bd1e594bec33bc07a7829c51cadcc810ac3da7df
git dirty: false

Linux binary:
c72b41dda3356b6f61be9fd43f7c89dbf30b6f223a90760b71dad43be728aa3c

Windows binary:
f96db44e8aed5ffb8ebf9b9e302fe006cba38eccfdfe145aa57c2c32c1079e3a

Linux tar.gz:
c8ec8e5d21e96562976ba3ede113cd1bf650fa8c60229f33e8babea7837971c3

Windows zip:
d636c36afd33941bc37e177c6bc54a5e94a537e1ff00d3ed7e6da5d0d9b1cf79

Release manifest:
7269ef919e40f80e302b20fa93dc09e3d51a1c73572345965b23294c274095e5
```

## Next requirement

Bind self-evolution experience transitions to supersession and conflict invariants:

1. prevent multiple active versions in the same scope;
2. require superseding experience to exist and match kind/scope;
3. reject expired experience activation;
4. preserve rollback history;
5. test replacement and conflicting activation.
