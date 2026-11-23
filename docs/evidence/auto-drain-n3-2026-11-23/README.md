# Four-arm N=3 after auto-drain and fast default — 2026-11-23

Suite `evals/real-tasks-v1.json` (8 cases), N=3, model `deepseek/deepseek-flash`
via `http://127.0.0.1:8787/v1`. Binary under test:
`~/.local/bin/1agent` sha256 `b200a848affb98367f31…` (the benchmark build; the
final installed binary `1da41f5d…` differs only by an empty-read-root delivery
fallback that this suite does not exercise).

Command:

```bash
PATH=/home/skynet/.local/bin:$PATH python3 scripts/phase4_bench.py \
  --runs 3 --arms full,core7,fast,pi \
  --out docs/evidence/auto-drain-n3-2026-11-23
```

## Aggregate

| Arm | Neutral | Status met | Model calls | Tool calls | Wall ms | Fixed tokens | Prompt tokens |
|---|---:|---:|---:|---:|---:|---:|---:|
| full | 23/24 (95.83%) | 23/24 | 7.0 | 6.0 | 18,429 | 8,972 | 83,927 |
| core7 | 24/24 (100%) | 20/24 (83.33%) | 5.0 | 4.0 | 16,419 | 4,478 | 32,056 |
| fast | **24/24 (100%)** | **24/24 (100%)** | **3.0** | **2.0** | **3,947** | **813** | **4,639** |
| pi | 24/24 (100%) | n/a | 4.0 | 3.0 | 5,463 | 595 | 7,506 |

Per-case neutral completion:

| Case | full | core7 | fast | pi |
|---|---:|---:|---:|---:|
| bug-fix-last-index | 3/3 | 3/3 | 3/3 | 3/3 |
| config-retries | 3/3 | 3/3 | 3/3 | 3/3 |
| data-processing-sales | 3/3 | 3/3 | 3/3 | 3/3 |
| research-compare | 3/3 | 3/3 | 3/3 | 3/3 |
| long-task-plan | 3/3 | 3/3 | 3/3 | 3/3 |
| broken-json | 3/3 | 3/3 | 3/3 | 3/3 |
| checksum-file | **2/3** | 3/3 | 3/3 | 3/3 |
| directory-summary | 3/3 | 3/3 | 3/3 | 3/3 |

## Fast delivery receipts

All **24/24 fast receipts are `accepted`**, with **0 failed checks** and **no
`journal:terminal_write_unpinned`**. The post-run snapshot root is the task
workspace, not the configured `/home/skynet` read root.

## The one full-arm miss is environmental

`full/checksum-file/run-3` wrote the exact 8-byte file (`exact_bytes=true`) but
the model chose `terminal_create` to compute the hash. The host's
`skynet-term-session` was at its 128-active-session cap, so the tool returned:

```
tool failed: skynet-workshop-session exited exit status: 1:
skynet-term-session: create_failed: session_limit：活动会话最多 128 个
```

The model retried `edit_file`/`write_file`/`request_tools` and exhausted its
step budget, so `reports_hash` was not met. This is a host session-cap failure,
not a change in the full arm (the arm passes `--no-fast` and does not use the
auto-drain path). The fast arm was unaffected; its two `checksum-file` runs with
one failed tool each still completed and were accepted.

## Fast receipts and auto-drain

The fast arm used the installed binary's explicit `--fast`; the CLI auto-drain
processed each receipt before the process exited. `scripts/phase4_bench.py`
now reads `HOME/acceptance/done/<run>.json` first and falls back to
`1agent accept --queue` for older binaries. Every fast run's
`result.json` has `delivery_status=accepted`.
