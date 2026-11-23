# Phase 4 — three-arm real-task measurement

- Suite: `real-tasks-v1` (8 cases), N=3
- Model: `deepseek/deepseek-flash` via `http://127.0.0.1:8787/v1`
- Started: 2026-11-23T11:01:06.772756+00:00
- Finished: 2026-11-23T11:05:19.354409+00:00
- 1agent: `/home/skynet/.local/bin/1agent` sha256 `3e6f3dcdbd6f…`
- Pi: `1.5.2`

Completion is neutral (workspace files plus cited answer terms), not 1agent's structured rubric.
User corrections are not measured: every run is non-interactive.
Pi has no structured delivery status; its `1agent status met` cell is not applicable.
1agent status met compares the answer status with the suite's expected status (research-compare expects conditional).
For the fast arm, status met uses the post-run acceptance receipt (`accepted` or expected `conditional`).
1agent fixed tokens come from step-0 metrics; Pi fixed tokens are estimated from its system prompt.

| Arm | Neutral completion | 1agent status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |
|---|---|---|---|---|---|---|---|---|---|
| fast | 24/24 | 24/24 | 3.0 | 2.0 | 3872.0 | 790.0 | 813.0 | 4542.5 | 454.0 |
| pi | 24/24 | — | 4.0 | 3.0 | 6189.5 | 653.0 | 596.5 | 7509.5 | 586.0 |

## Per-case neutral completion

| Case | fast | pi |
|---|---|---|
| bug-fix-last-index | 3/3 | 3/3 |
| config-retries | 3/3 | 3/3 |
| data-processing-sales | 3/3 | 3/3 |
| research-compare | 3/3 | 3/3 |
| long-task-plan | 3/3 | 3/3 |
| broken-json | 3/3 | 3/3 |
| checksum-file | 3/3 | 3/3 |
| directory-summary | 3/3 | 3/3 |
