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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: c5656b4492d412a7345b0ef269f1d4bc15a67061
git dirty: false

Linux binary:
87883925da3d1705cdce966dfa31b7613f801014e3f1f552009d4f19d6a6661a

Windows binary:
e520ead5b2b9424a74cae68d3c34231652daae002dc27d5807f0b2bd5e2bf7bf

Linux tar.gz:
61aed45d1f593e81ad09f15f4579701b047571af3f710912f6a79d45115c3813

Windows zip:
372e035086d9b303f2301257ea32b823b691f2c9e4fb67805f63fd3eff42489a

Release manifest:
77ccaabdc25d20f9cbfbb387fe214a932da9804a7250df0594c54ac664246474
```

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
