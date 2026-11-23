# Phase 4 — three-arm real-task measurement

- Suite: `real-tasks-v1` (8 cases), N=3
- Model: `deepseek/deepseek-flash` via `http://127.0.0.1:8787/v1`
- Started: 2026-11-23T06:44:01.915676+00:00
- Finished: 2026-11-23T07:00:18.968794+00:00
- 1agent: `/home/skynet/.local/bin/1agent` sha256 `d20468cd8e75…`
- Pi: `1.5.2`

Completion is neutral (workspace files plus cited answer terms), not 1agent's structured rubric.
User corrections are not measured: every run is non-interactive.
Pi has no structured delivery status; its `1agent status met` cell is not applicable.
1agent status met compares the answer status with the suite's expected status (research-compare expects conditional).
1agent fixed tokens come from step-0 metrics; Pi fixed tokens are estimated from its system prompt.

| Arm | Neutral completion | 1agent status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |
|---|---|---|---|---|---|---|---|---|---|
| full | 24/24 | 17/24 | 6.0 | 5.0 | 17553.0 | 882.5 | 8971.0 | 70537.0 | 2899.5 |
| core7 | 24/24 | 21/24 | 5.0 | 3.0 | 16522.0 | 867 | 4476.5 | 24965.0 | 2837.0 |
| pi | 24/24 | — | 3.5 | 3.0 | 5434.0 | 522.0 | 597.0 | 6836.5 | 541.5 |

## Per-case neutral completion

| Case | full | core7 | pi |
|---|---|---|---|
| bug-fix-last-index | 3/3 | 3/3 | 3/3 |
| config-retries | 3/3 | 3/3 | 3/3 |
| data-processing-sales | 3/3 | 3/3 | 3/3 |
| research-compare | 3/3 | 3/3 | 3/3 |
| long-task-plan | 3/3 | 3/3 | 3/3 |
| broken-json | 3/3 | 3/3 | 3/3 |
| checksum-file | 3/3 | 3/3 | 3/3 |
| directory-summary | 3/3 | 3/3 | 3/3 |
