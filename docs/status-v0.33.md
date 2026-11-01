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
