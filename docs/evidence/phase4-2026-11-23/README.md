# Phase 4 — three-arm real-task measurement (2026-11-23)

Status: **baseline measurement complete; acceptance-fix comparison lives in
`../phase4-acceptance-fix-2026-11-23/`.**

## Method

- Suite: `evals/real-tasks-v1.json` (8 real tasks), N=3.
- Model: `deepseek-flash` via `http://127.0.0.1:8787/v1` (headroom proxy).
- Arms:
  - `full`: 1agent CLI with the full tool surface (37 tools).
  - `core7`: 1agent with the Core 7 Act surface (5 tools plus virtual
    `describe_self`/`request_tools`), reproduced with `--allowed-tools`.
  - `pi`: Pi 1.5.2 default tools inside a read-only Docker container.
- Completion is a neutral check of the workspace plus cited answer terms, so
  all arms are scored the same way. `1agent status met` compares the answer
  status with the suite's expected status (`research-compare` expects
  `conditional`).
- User corrections are not measured: every run is non-interactive.

Command:

```bash
python3 scripts/phase4_bench.py --runs 3 \
  --out docs/evidence/phase4-2026-11-23
```

## Three-arm result

| Arm | Neutral completion | 1agent status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |
|---|---|---|---|---|---|---|---|---|---|
| full | 24/24 | 16/24 | 6.0 | 5.0 | 16,412.5 | 858.0 | 8,879.5 | 69,110.0 | 2,708.0 |
| core7 | 24/24 | 18/24 | 6.0 | 5.0 | 17,173.0 | 588.0 | 4,385.0 | 33,164.0 | 3,174.5 |
| pi | 24/24 | — | 4.0 | 3.0 | 5,037.0 | 554.0 | 593.5 | 7,563.5 | 560.0 |

All arms produced the correct workspace result on all 24 attempts. Core 7
halved fixed and prompt tokens but did not reduce median wall time. Pi was
about 3.3× faster and used about 1/9 of the prompt tokens, but has no
acceptance, evidence or work-ledger status.

## Product eval N=3

The product's own suite run with the same config:

- overall pass rate: **20/24 = 83.33%**
- pass^3: **6/8 = 75%**
- pass at least once: **8/8 = 100%**
- failures: `long-task-plan` 1/3, `checksum-file` 1/3
- failure classes: `evidence` 2, `generation` 2

Artifacts: `product-eval/repeat-report.json`, `product-eval/repeat-1..3/`.

## Failure analysis

The product-eval failures and the neutral-benchmark misses point to the
completion gate, not task capability:

- `terminal_exec`, `terminal_create` and `terminal_output` were treated as
  writes even for `jq`, `ls`, `sha256sum` and `python -m json.tool`; a
  read-only answer therefore required acceptance, and a later read-only shell
  command could clear an accepted receipt.
- Models wrote `independent:…` or prose verifier IDs and were rejected by the
  `verifier:`/`agent:` prefix check.
- After a later `acceptance_gate` call succeeded, some answers still reported
  `conditional` because of earlier failed attempts.
- Result criteria sometimes differed from contract criteria by case or
  whitespace.
- Some accepted receipts arrived on the last step; the final answer repair
  then timed out or had no step left.

The follow-up acceptance fix and its comparison are recorded in
`docs/evidence/phase4-acceptance-fix-2026-11-23/`.

## Limits

- N=3 and eight tasks: directional, not statistical significance.
- Neutral checks judge the result, not process quality or maintainability.
- 1agent runs on the host; Pi runs in Docker, so wall time is directional.
- One model and one endpoint.
- No human blind review or user-correction measurement.
