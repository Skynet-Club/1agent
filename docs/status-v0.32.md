# Status v0.32.0

Date: 2026-11-02
Status: provider route budget constraints implemented

## Changes since v0.31.0

- Added two optional `RunBudget` fields:
  - `max_provider_p95_latency_ms`;
  - `max_unit_cost_micros_per_k`.
- Router applies both as hard filters before objective ranking.
- Rejections are explicit:
  - `provider_p95_latency_exceeded`;
  - `unit_cost_exceeded`.
- Added CLI overrides to `run` and `route-plan`.
- The initial journaled task keeps the effective limits.
- A no-candidate route remains a planning result rather than a model call.

## Verification

| Check | Result |
|---|---|
| Latency budget hard filter | passed |
| Unit-cost budget hard filter | passed |
| Combined rejection reasons | reported |
| Runtime budget mapping | implemented |
| Unit tests | 74 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 2711406ac55aa8df12193b812191a60ddd6f1edb
git dirty: false

Linux binary:
17228f90789df947f20b9cb587759f966ad7874c1cb671cacdbe4899559732ea

Windows binary:
264b1aa5df20ed166c2dd1efece93f3bbcc53e67d8b488201f84da8a09cfde60

Linux tar.gz:
7b3956887e2459d1961c94f076d36d2776e6e8e8c016bd5717b7451750809578

Windows zip:
53e839be031cea947b76b33486724c49335d8ad02a0f827362e446da9dadd660

Release manifest:
6372b11e2be730ff04bd6316ba72d5a986701860cb3a21fef83d0de3d083a47e
```

## Semantics and limitations

- `max_provider_p95_latency_ms` compares the operator-supplied provider p95 estimate; it is not a
  per-request deadline.
- `max_unit_cost_micros_per_k` compares combined input/output metadata price; it is not an actual
  invoice calculation.
- Token-count-dependent per-run cost ceilings remain future work.
- Provider metadata is not yet calibrated from live telemetry.

## Next requirement

Record route planning and execution decisions in a machine-readable runtime trace:

1. Persist effective objective and budget source.
2. Include rejected routes in a compact route-plan journal event.
3. Keep secrets and endpoint credentials out of the journal.
4. Replay route decisions without executing models.
5. Audit execution selection against the planned candidate set.
