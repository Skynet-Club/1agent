# Phase 4 — three-arm real-task measurement

- Suite: `real-tasks-v1` (8 cases), N=3
- Model: `deepseek/deepseek-flash` via `http://127.0.0.1:8787/v1`
- Started: 2026-11-23T09:11:04.268862+00:00
- Finished: 2026-11-23T09:17:08.803639+00:00
- 1agent: `/home/skynet/.local/bin/1agent` sha256 `e35b3ad9b8cf…`
- Pi: `1.5.2`

Completion is neutral (workspace files plus cited answer terms), not 1agent's structured rubric.
User corrections are not measured: every run is non-interactive.
Pi has no structured delivery status; its `1agent status met` cell is not applicable.
1agent status met compares the answer status with the suite's expected status (research-compare expects conditional).
For the fast arm, status met uses the post-run acceptance receipt (`accepted` or expected `conditional`).
1agent fixed tokens come from step-0 metrics; Pi fixed tokens are estimated from its system prompt.

| Arm | Neutral completion | 1agent status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |
|---|---|---|---|---|---|---|---|---|---|
| fast | 20/24 | 20/24 | 5.5 | 5.0 | 6854.5 | 844 | 681.0 | 9609.0 | 845.0 |
| pi | 24/24 | — | 4.0 | 3.0 | 6048.5 | 599.0 | 594.5 | 7476.0 | 681.5 |

## Per-case neutral completion

| Case | fast | pi |
|---|---|---|
| bug-fix-last-index | 3/3 | 3/3 |
| config-retries | 3/3 | 3/3 |
| data-processing-sales | 3/3 | 3/3 |
| research-compare | 2/3 | 3/3 |
| long-task-plan | 3/3 | 3/3 |
| broken-json | 2/3 | 3/3 |
| checksum-file | 2/3 | 3/3 |
| directory-summary | 2/3 | 3/3 |
