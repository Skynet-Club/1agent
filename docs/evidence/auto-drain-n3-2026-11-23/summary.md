# Phase 4 — three-arm real-task measurement

- Suite: `real-tasks-v1` (8 cases), N=3
- Model: `deepseek/deepseek-flash` via `http://127.0.0.1:8787/v1`
- Started: 2026-11-23T14:44:25.586198+00:00
- Finished: 2026-11-23T15:02:21.160643+00:00
- 1agent: `/home/skynet/.local/bin/1agent` sha256 `b200a848affb…`
- Pi: `1.5.2`

Completion is neutral (workspace files plus cited answer terms), not 1agent's structured rubric.
User corrections are not measured: every run is non-interactive.
Pi has no structured delivery status; its `1agent status met` cell is not applicable.
1agent status met compares the answer status with the suite's expected status (research-compare expects conditional).
For the fast arm, status met uses the post-run acceptance receipt (`accepted` or expected `conditional`).
1agent fixed tokens come from step-0 metrics; Pi fixed tokens are estimated from its system prompt.

| Arm | Neutral completion | 1agent status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |
|---|---|---|---|---|---|---|---|---|---|
| full | 23/24 | 23/24 | 7.0 | 6.0 | 18429.0 | 1122.0 | 8972.0 | 83927.0 | 3451.5 |
| core7 | 24/24 | 20/24 | 5.0 | 4.0 | 16418.5 | 977.5 | 4477.5 | 32056.0 | 3046.5 |
| fast | 24/24 | 24/24 | 3.0 | 2.0 | 3946.5 | 694.5 | 813.0 | 4639.0 | 487.0 |
| pi | 24/24 | — | 4.0 | 3.0 | 5462.5 | 768.0 | 595.0 | 7505.5 | 526.5 |

## Per-case neutral completion

| Case | full | core7 | fast | pi |
|---|---|---|---|---|
| bug-fix-last-index | 3/3 | 3/3 | 3/3 | 3/3 |
| config-retries | 3/3 | 3/3 | 3/3 | 3/3 |
| data-processing-sales | 3/3 | 3/3 | 3/3 | 3/3 |
| research-compare | 3/3 | 3/3 | 3/3 | 3/3 |
| long-task-plan | 3/3 | 3/3 | 3/3 | 3/3 |
| broken-json | 3/3 | 3/3 | 3/3 | 3/3 |
| checksum-file | 2/3 | 3/3 | 3/3 | 3/3 |
| directory-summary | 3/3 | 3/3 | 3/3 | 3/3 |
