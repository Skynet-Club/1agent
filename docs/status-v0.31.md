# Status v0.31.0

Date: 2026-11-02
Status: offline route planning implemented

## Changes since v0.30.0

- Added `route-plan --config <file> --task <text>`.
- Uses the same `Router::plan` implementation as runtime routing.
- Reports:
  - inferred cognitive mode;
  - selected route objective;
  - ranked candidates and reasons;
  - rejected candidates with explicit hard-filter reasons;
  - provider credential and endpoint readiness.
- Hard-filter rejection reasons are:
  - unsupported mode;
  - risk exceeded;
  - tools unsupported;
  - structured output unsupported;
  - remote disallowed.
- `--json` emits the complete planning report.
- Route planning performs no network calls.

## Verification

| Check | Result |
|---|---|
| Offline plan execution | passed |
| Remote-disabled rejection | reported |
| Tool-capability rejection | reported |
| Credential readiness | reported |
| Planner/runtime shared implementation | enforced |
| Unit tests | 74 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- Tool requirement is currently based on the configured registry being non-empty.
- Readiness is configuration-level, not live provider conformance.
- Route-plan data is not persisted to a run journal because no run occurs.
- Quality/latency/cost metadata remains uncalibrated.

## Next requirement

Connect route policy to task intent:

1. Add optional latency and cost budgets to task requests.
2. Select a default objective from explicit task fields, not hidden heuristics.
3. Reject tasks whose budget excludes every candidate.
4. Preserve user override.
5. Journal the policy source and effective objective.
