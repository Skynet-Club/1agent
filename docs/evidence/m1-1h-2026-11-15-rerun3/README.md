# 0.98 1h M1 — rerun3 (passing)

Date: 2026-11-15  
Model: `deepseek-primary` / `deepseek-flash`  
Suite: `real-tasks-v1` (8 cases, 30 runs)  
Binary at run start: `d87e1ead8e0f3bf8424036dc6ec965026cb196cbbc3bcc5cf13c5f15a87a4e66`

## Result

- Task completion: **239 / 240 = 99.58%** (gate ≥97%) → pass
- Suite completion: **96.66%**
- Completed-run replay: **239 / 239 = 100%**
- Crashed runs: **0 / 30**
- Cost: 10,659,926 micros
- Report SHA-256: `9e7333e4b23447e865e6ebe7bd25cdad6fb95940d3203861500c501f21e3940e`

The single miss was `bug-fix-last-index`, which returned `completed` /
`conditional`; it did not trip a hard gate. No budget-exhaustion or provider
error appeared in this run.

## Command

```bash
PI_DEEPSEEK_API_KEY="$(jq -er '.deepseek.key' /home/skynet/.pi/agent/auth.json)" \
./target/release/first-principles-agent benchmark-run \
  --suite evals/real-tasks-v1.json --runs 30 \
  --config configs/pi-deepseek.json --allow-network \
  --artifacts-dir docs/evidence/m1-1h-2026-11-15-rerun3 \
  --timeout-seconds 240 --max-cost-micros 20000000 \
  --min-task-completion-bps 9700 --min-completed-replay-bps 10000 --json
```

## Artifacts

- `../m1-1h-2026-11-15-rerun3.json`: gate report
- `run-0000` … `run-0029`: per-run journals and `cases.jsonl`
- `../m1-1h-2026-11-15-rerun3.stderr`: CLI stderr

## Note on the binary

M1 was produced by the release binary from the tree **before** the session
interaction changes in `src/tui.rs` / `src/main.rs`. Those changes affect the
TUI only; `benchmark-run` does not use them. The M1 source tree is otherwise
the same 0.98 tree. A final release build will have a different binary hash;
record that hash with the release artifact.
