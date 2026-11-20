# Super Slice 1 — faster, cleaner, smarter (2026-11-21)

Status: **landed; profile guidance, environment manifest, step metrics, cache-safe
nudges, read cache/coalescing/circuit breaker, and large-payload receipts**

Feature commits: `1d0b164` (task profiles, step metrics, tool cache),
`fd84da3` (context breakdown, recitation, CLI provider-attempt budget fix),
`c0b0db3` (multi-tool page classification, page checklist, audit repair),
`5fc7830` (verification reserve, audit recheck, page acceptance re-plan) and
`c067b46` (verification-budget warning).

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
     `circuit_open` observation.
   - This targets repeated **read-only** calls when the workspace has not
     changed. The 59-step baseline's browser audits followed page edits, so
     they were legitimate re-verification and must stay ordered/uncached.
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

## Live smoke on the first rc.13 build

Release binary SHA-256 of this smoke:
`5e1b38216eb5a0c809d30605e108096f70a152e6a4438e74c5c7ba6a8884c976`
Final installed rc.13 (context breakdown, recitation, CLI budget fix):
`010bb27b84c02c66ec6972b4e3eb8cdd884fe83959e84918fb31d4ac0620887f`

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

## Live artifact delivery (T1-like, not the same lark task)

Run: `target/release/first-principles-agent` (rc.13 + context breakdown/recitation),
single-file HTML page, max 20 steps.

| Metric | Value |
|---|---:|
| Wall clock | **45.85s** |
| Loop steps / model responses | 7 / 8 (1 repair) |
| Tool calls | 6 |
| Model time / tool time | 35,601ms / 1,832ms |
| Context tokens | 11.4k → 13.5k |
| Input / output tokens | 134,561 / 10,079 |
| Cache hit rate | 88.2% |
| Duplicate calls | 0 |
| Artifact | 4,119 bytes, sha256 `cf12c2ab…` |
| Verification | preview HTTP 200; browser audit all_pass; acceptance accepted |

Independent render: Google Chrome headless at 390×844 and 1440×900, exit 0.
Screenshots: `live-html-mobile.png`, `live-html-desktop.png`.

This is **not** the original 59-step lark task; it is a smaller artifact task.
It shows the harness can deliver a verified page in 7 loop steps with no
duplicate calls, but the real T1 rerun is still required before claiming a
specific speedup on the original task.

## Original T1 rerun (same core task, isolated output path)

The original 59-step task was rerun with the final rc.13 harness and an
isolated output path (`/tmp/1agent-t1-rerun9/index.html`; the original
`/home/skynet/work/join-us` was not modified).

| Metric | Baseline | Rerun | Change |
|---|---:|---:|---:|
| Wall clock | 365.0s | **187.8s** | **−48.6% (1.94×)** |
| Model responses | 59 | **30** | −49.2% |
| Loop steps | 59 | **29** | −50.8% |
| Tool calls | 68 | **42** | −38.2% |
| Input tokens | 4,946,706 | **1,801,351** | −63.6% |
| Avg input tokens | 83,842 | **60,045** | −28.4% |
| Max input tokens | 133,893 | **93,796** | −29.9% |
| Tool time | 71.6s | 5.1s | −92.9% |
| Final status | conditional | conditional | parity |
| Browser audit | all_pass=true | all_pass=true | parity |
| Acceptance gate | not attempted | attempted, conditional | better honesty |

The rerun found 8 small tap targets, fixed them, re-audited to
`all_pass=true`, then called `acceptance_gate` twice. It returned conditional
because the requested `ui-ux-pro-max` skill is not installed; the run reports
that gap instead of claiming completed. The baseline was also conditional.
This is the **quality-parity + honest-blocker** result; earlier attempts
without the classifier/audit fixes stopped with unfixed tap targets and were
not counted as wins.

Artifact: 34,520 bytes, sha256 `cd75b40a…`; screenshots `t1-rerun-mobile.png`
and `t1-rerun-desktop.png`; full metrics `t1-rerun.json`.

### What made the rerun both faster and not dumber

1. The original task was initially misclassified as **fast** because it ends in
   a page. The classifier now treats `lark-cli`, a URL plus an installed skill,
   `深度思考`, `验收`/`质检` as multi-step signals; the task is **deep**. This is
   the anti-降智 guard: a page-shaped artifact does not make a multi-tool task
   simple.
2. A page-delivery checklist is injected once for page tasks: write →
   preview → browser open → audit at 390/768/1440 → fix and re-audit →
   acceptance gate.
3. A failed browser audit now adds a repair checkpoint instead of letting the
   model answer conditional with fixable defects.
4. The CLI provider-attempt budget is derived from `max_steps` (see below), so
   a 40-step run is not silently capped at 8 successful model calls.

### Three-repeat consistency (n=3)

After fixing the CLI provider-attempt default (see below), the same task ran
three times with the installed rc.13 binary:

| Metric | Median | Range |
|---|---:|---:|
| Wall clock | **35.1s** | 29.3–44.1s |
| Loop steps | **6** | 6–9 |
| Tool calls | **6** | 5–8 |
| Model time | 33.3s | 20.2–42.2s |
| Tool time | 1.85s | 1.84–1.87s |
| Max context tokens | 13.5k | 13.4k–15.5k |
| Cache hit rate | 88.6% | 87.2–92.6% |
| Duplicate calls | **0** | 0 |

All three runs produced an accepted artifact (2 `completed`, 1 `conditional`);
none exhausted the provider-attempt budget. Artifact sizes 3.5–4.5KB.
Details: `live-html-n3.json`; rep1 artifact/screenshot/output in
`live-html-n3-rep1.*`.

### Provider-attempt bug found by the repeats

The first n=3 attempt (before the fix) produced one `needs_evidence` run with
`provider attempt budget exhausted`. Root cause: `1agent run` defaulted
`--max-provider-attempts` to 8, and that counter increments for **successful**
model calls too, so `--max-steps 20` was silently capped at 8 model calls.
The TUI already derived the provider-attempt budget from `max_steps`; the CLI
now does the same (`tui_provider_attempt_budget`). This is a real harness fix,
not a model-quality claim.

## Gates

- `cargo fmt --check`: clean.
- `cargo clippy --offline --all-targets -- -D warnings`: clean.
- `cargo test --lib`: **1,387 passed / 0 failed / 4 ignored**.
- `cargo test --bins`: **68 passed / 0 failed**.
- Full test suite with the known CLI-session test skipped:
  **1,857 passed / 0 failed / 13 ignored**.
- Eval `evals/core.json`: **8/8 PASS**, hard-gate failures 0, rubric 6/6.
- `scripts/verify-claims.sh`: **290 checked / 266 verified / 16 partial /
  8 no evidence**, structure only.

Known pre-existing failure: `cli_tui_agents_refresh_uses_configured_profiles`
(tests/cli_e2e.rs:497). It is the skynet-term 128-active-session limit, not a
code regression; the full gate log stops there. See
`gate-verify-known-cli-failure.txt`.

## What this slice does not prove

- The T1 rerun is **n=1** and uses an isolated output path. The full T1–T12
  suite and repeat protocol do not exist yet; the profile is guidance, not a
  hard budget.
- `StepModelMetrics` covers the main loop. Repair and budget-finalization calls
  are still recorded in `ModelResponded` but not in `StepModelMetrics`, so
  model-time totals can undercount by one repair call.
- The read cache is per-run and side-effect-free only; writes clear it. There is
  no cross-run cache. It would **not** have removed the baseline's browser
  audits, because the page changed between audits and `browser` is ordered.
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
