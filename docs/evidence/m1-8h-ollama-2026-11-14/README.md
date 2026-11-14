# 8-hour local-model runtime soak

- Date: 2026-11-14
- Provider: local Ollama `qwen3-8b-16k` at `http://127.0.0.1:11434/v1`
- Suite: `real-tasks-v1` v1.2 (8 cases per run, 8 runs per batch)
- Runner: `target/debug/first-principles-agent benchmark-run`, batches until the
  8-hour wall clock; started 2026-11-14 07:17 CST, finished 15:44 CST
- Actual duration: **30,411 s = 8h 26m 51s**
- Batches: 7 × 8 runs = **56 iterations / 448 tasks**
- Quality gates were disabled for this soak (`--min-completion-bps 0
  --min-task-completion-bps 0`); crash-free and replay gates were kept
  (`--min-crash-free-bps 10000 --min-replay-bps 10000
  --min-completed-replay-bps 10000`).

## Runtime result

- Crashed runs: **0 / 56** → crash-free **100%**
- Completed cases: **234**; completed-run replay: **234/234 = 100%**
- Audit failures: **0**; replay failures: **0**
- Stop reasons: 234 completed, 185 needs_evidence, 29 budget_exhausted
- Every batch exited 0 (the stability gates passed); `passed=true` in the batch
  reports means stability, not task quality.

## Model-quality result (not an M1 release claim)

- Task-level pass: **95/448 = 21.20%**
- Answer statuses: 231 completed, 3 conditional, 214 needs_evidence
- This is expected for a local 8B model and is **not** comparable to the
  DeepSeek M1 run. The M1 quality evidence remains the passing 20-run DeepSeek
  regression (task 93.12%, suite 55%, M5 100%).

## Artifacts

- `summary.json`: aggregate totals and per-batch metrics.
- `batch-XX.json` / `batch-XX.stderr` / `batch-XX.exit`: per-batch reports.
- `cases-all.jsonl`: all 448 case results.
- `journals.sha256`: digest of every raw journal under
  `/tmp/1agent-8h-ollama-artifacts` (raw journals are kept there, not in git,
  because they contain tool output and model text).
- `run.log`: batch start/end timestamps and elapsed seconds.

## Limits

- Local model quality is low by design; this proves runtime stability, not M1.
- Ollama runs from `/tmp` (no sudo/systemd); a reboot would clear the model and
  stop the server. The server is a nohup process, not a service.
- One provider/model only.
