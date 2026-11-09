# Status v0.33.0

Date: 2026-11-02
Status: route-plan journaling and replay implemented

## Changes since v0.32.0

- Added `RoutePlanned` journal event containing:
  - complete route request;
  - ranked candidates and reasons;
  - rejected candidates and hard-filter reasons;
  - source of objective, latency budget and unit-cost budget.
- Value sources are:
  - `runtime_default`;
  - `task_request`;
  - `cli_override`;
  - `unset`.
- Route plans are written before the no-route check, so failed planning remains auditable.
- Replay state now includes `route_plan` and `route_policy_sources`.
- Audit rejects an executed `RouteSelected` route that is absent from the preceding plan.
- Journals created before route planning remain readable and are not retroactively rejected.
- Route-plan CLI output now includes structured policy sources.

## Verification

| Check | Result |
|---|---|
| Plan persisted before route selection | passed |
| Route plan replay | passed |
| Policy source persistence | passed |
| Selected route must be planned | passed |
| Out-of-plan selection rejection | passed |
| Unit tests | 75 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 3ca189a7b80abbec13c463f86a70ca2d40e5e3ee
git dirty: false

Linux binary:
f6b48453bf64cb128c506550e032a3482014cdad4efb2f17d24e368d7c9af2a1

Windows binary:
b67eca7662d558cfcf2b9edb12c13241027ba6e9a9bb5b5809a76248864513d3

Linux tar.gz:
dd77fb7d1b579b9af78c728d16ace94fc21fbbb43dae5fedd2ea6fbd4aeb0bb2

Windows zip:
68af3c645d22ae4b7fd5f568111ab89966c952745f8b1ead39cb7e43850e12d0

Release manifest:
4b3606bc23a6ca629555a45d59cf37690a62228f48da87d9a78becb7aa223d8b
```

## Known limitations

- Resume currently derives one new plan from the restored request and runtime policy.
- Audit does not prove that fallback order was followed, only that each selected route existed in
  the plan.
- Route-plan events may contain provider IDs/models but never credentials or endpoint secrets.
- Planning metadata quality remains dependent on operator-supplied estimates.

## Next requirement

Strengthen checkpoint/resume equivalence for routing:

1. Bind checkpoints to route plan digest and effective policy sources.
2. Reject resume when the caller changes hard routing policy.
3. Persist objective overrides explicitly in run identity.
4. Add an interruption/resume route-equivalence test.
5. Keep old checkpoints readable with an explicit legacy status.
