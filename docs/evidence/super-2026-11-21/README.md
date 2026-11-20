# Super Slice 1 — faster, cleaner, smarter (2026-11-21)

Status: **landed; profile guidance, environment manifest, step metrics, cache-safe
nudges, read cache/coalescing/circuit breaker, and large-payload receipts**

Feature commit: `1d0b164` (`feat(1.2): add task profiles, step metrics and tool cache`).

## What changed

1. **Task profiles (guidance, not a hard cap)**
   - `src/task_profile.rs` classifies `fast`, `standard`, `deep` from the
     objective, deliverable, constraints and success criteria.
   - The profile is journaled as `TaskProfileSelected` and injected once as a
     stable system message. Fast profiles get checkpoints at steps 12/20;
     standard keeps 30/40; deep gets 50/65.
   - Explicit `RunBudget.max_steps` still wins. Hard profile budgets are
     deliberately deferred until the T1–T12 baseline exists.
2. **Environment manifest**
   - One compact system message at run start: workspace, OS/arch, shell, git
     root, available commands, permission summary.
   - `ONEAGENT_ENV_MANIFEST=0` disables it.
3. **Step metrics**
   - `StepModelMetrics`: step, profile, provider/model, turn kind, prep/model
     time, optional TTFT, input/output/cache tokens, cost, estimated context
     tokens, effective max steps, requested tool calls.
   - `StepToolMetrics`: tool calls, tool time, groups, ok/failed/denied,
     cache hits, circuit opens.
   - TUI run metrics now show `模型 … · 工具 …` in addition to tokens/cache.
4. **Cache-safe efficiency nudges**
   - The old nudge appended to the system message, invalidating the cached
     prefix at steps 30/40. It is now a transient tail message.
5. **Tool-call cache and failure governance**
   - Identical side-effect-free calls are served from a per-run cache.
   - Any write/ordered tool clears the cache, so a stale read cannot survive a
     mutation.
   - Repeated identical failures return a cached failure once, then a
     `circuit_open` observation. This directly targets the 3 repeated browser
     audits and repeated missing-skill searches in the 59-step baseline.
   - `skill_search`, `skill_list`, `memory_recall`, `memory_why` and
     `session_search` are now parallel-safe so they can be cached/coalesced.
6. **Large payload receipts**
   - Tool content above 32,000 chars is written under
     `<journal-dir>/tool-results/<tool>-<sha16>.txt` and replaced in context
     with `{content_offloaded, path, sha256, bytes, chars, summary}`.

## Baseline: the real 59-step run

Journal: `~/.1a/journals/d990ae4b-923a-4c3f-bb15-19084b2b9823.jsonl`

| Metric | Value |
|---|---:|
| Wall clock | 365.0s |
| Model responses | 59 |
| Tool calls / observations | 68 / 68 |
| Tool ok / failed | 66 / 2 |
| Input tokens | 4,946,706 |
| Output tokens | 51,116 |
| Cache hit / miss tokens | 4,860,416 / 86,290 |
| Cache hit rate | 98.25% |
| Average input tokens/turn | 83,842 |
| Tool duration | 71,636ms (19.6% of wall) |
| Estimated model time | 293.4s (80.4%) |
| Tool schemas | 38 |
| Tool observation bytes | 175,729 |
| Duplicate call keys / extra calls | 1 / 2 |
| Repeated audit | `browser{action:"audit",widths:[390,768,1440]}` × 3 |
| Repeated audit duration | 2,777ms |

The baseline says the loop is **model-round-trip bound**, not tool-bound:
59 model responses account for about 80% of wall clock. The repeated audit and
the 38-tool schema load are real waste, but they are secondary to turn count.

## Live smoke on the installed rc.13

Release binary SHA-256:
`5e1b38216eb5a0c809d30605e108096f70a152e6a4438e74c5c7ba6a8884c976`

Journal: `/tmp/1agent-release-smoke/ca45d775-8e16-47eb-aed4-696d812679d0.jsonl`

| Metric | Value |
|---|---:|
| Result | completed |
| Model responses | 4 (3 loop steps + 1 repair) |
| StepModelMetrics / StepToolMetrics | 3 / 2 |
| Tool calls | 2 |
| Context tokens (loop steps) | 11,074 / 11,544 / 11,852 |
| Input / output tokens | 37,546 / 632 |
| Cache hit / miss tokens | 35,584 / 1,962 |
| Cache hit rate | 94.8% |
| Loop model time | 3,834ms |
| Tool time | 0ms |
| Profile | fast, confidence 75 |

The smoke confirms: profile event, environment manifest, step metrics, TUI
timings, and the release binary all work against the live provider. It did not
exercise the read cache because the model chose two different reads; the cache
and circuit breaker are covered by deterministic unit tests.

## Gates

- `cargo fmt --check`: clean.
- `cargo clippy --offline --all-targets -- -D warnings`: clean.
- `cargo test --lib`: **1,376 passed / 0 failed / 4 ignored**.
- `cargo test --bins`: **68 passed / 0 failed**.
- Full test suite with the known CLI-session test skipped:
  **1,846 passed / 0 failed / 13 ignored**.
- Eval `evals/core.json`: **8/8 PASS**, hard-gate failures 0, rubric 6/6.
- `scripts/verify-claims.sh`: **290 checked / 266 verified / 16 partial /
  8 no evidence**, structure only.

Known pre-existing failure: `cli_tui_agents_refresh_uses_configured_profiles`
(tests/cli_e2e.rs:497). It is the skynet-term 128-active-session limit, not a
code regression; the full gate log stops there. See
`gate-verify-known-cli-failure.txt`.

## What this slice does not prove

- T1–T12 fixed tasks do not exist yet. The profile is a hint, not a hard budget;
  no live 59-step rerun was done.
- `StepModelMetrics` covers the main loop. Repair and budget-finalization calls
  are still recorded in `ModelResponded` but not in `StepModelMetrics`, so
  model-time totals can undercount by one repair call.
- The read cache is per-run and side-effect-free only; writes clear it. There is
  no cross-run cache.
- The circuit breaker only covers parallel-safe tools; ordered tools still
  execute each time.
- Large-payload receipts change the model-visible content to a JSON receipt;
  consumers that expected the raw content must read the receipt path.
- `cost_micros` is 0 in the live smoke because the local provider manifest has no
  price table. Do not use it as a cost claim yet.

## Next slice

1. T1–T12 task definitions + baseline, then a profile hard-budget A/B.
2. Context ledger and compaction probe (C-01..C-04).
3. CodeAct/batch, async suspension, and the smartness gates (S-02..S-05).
