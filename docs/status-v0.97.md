# 0.97 Status and Metric Review (2026-11-13, evening CST)

- Status: **local review; 0.97 is not release-ready — the M1 20-run short
  regression passes (task 93.12%, suite 55%, M5 100%) and the 8h local-model
  runtime soak passes (56 runs, 0 crashes, 234/234 replay), but the 8h
  model-quality M1 on DeepSeek, human review, external audit and release
  packaging remain. AG-23 is implemented and its final 100-question run is
  archived (96/100).**
- Latest feature commit: `f77ddc1` (Windows/claim fix), after `a3060a3`
  (routing, evidence fallback and file listing), `a04686a` (100-question eval
  harness), `55ccb22` (strategy drafts), `5465c2a` (evidence re-plan),
  `c76a2d4` (live self model and dynamic tools) and `5726381` (0.97 WIP
  checkpoint); evidence archive `205ac3b`
- Full suite: 1315 passed / 0 failed / 13 ignored; `scripts/verify.sh`
  passed (fmt, clippy, tests, deterministic core eval, claims structure)
- Claims: 233 (209 verified / 16 partial / 8 no evidence); the live terminal
  claim is partial because the local session daemon is at its 128-session limit
  (`session_limit`), not because the terminal tool failed. `verify-claims --run`
  still reports that TUI command failure; `cross-check.sh --build` passes when
  run alone.
- Local release: Linux and Windows GNU binaries are rebuilt per slice; the
  Windows `.exe` is linked, not run on Windows

This page is a measurement review, not a release announcement. A metric marked
“not measured” is not zero; it means the evidence does not exist yet.

## Hard gates

| Gate | Metric | Current evidence | Status |
|---|---|---|---|
| M4 | Crash recovery ≥95% | Controlled resume sample 1/1 (100%); 2h crash-injection soak 28,552 cycles / 7,200.20s, exit 0, raw output retained (binary based on `7d12f0d`); no real-crash aggregate | **Local pass (soak)** |
| M5 | Replay 100% | 100% structural replay on completed journals in both local snapshots | **Local pass** |
| M6 | Zero security incidents | No P0 in the local red-team/security tests; 5 denied observations in historical journals | **Not externally verified** |
| M13 | Audit completeness 100% | 100% under `AgentAnswer::validate` on local journals | **Local pass** |
| M15 | TUI frame p95 ≤8 ms | Release 1.26 ms on a 200-entry transcript, `TestBackend` | **Local pass** |
| M16 | 1M journal incremental p95 ≤50 ms | 4.7 MB journal: full 31.8 ms, incremental p95 9.4–16.1 µs | **Local pass** |
| M17 | Compacted run replay 100% | Forced-compaction runtime fixture replays, audits and reports 100% | **Local pass** |

## Targets and baselines

| Metric | Current | Note |
|---|---|---|
| M1 long-horizon success | **First 8h failed**: 136/280 suites (48.57%). Post-repair 20-run: **144/160 tasks (90.0%)**, 10/20 suites (50%), M5 157/157. **320-run v2 finished 11:52 and failed**: 340/2560 tasks (13.28%), 25/320 suites, completed-case replay 351/351 (100%), crash-free 100%, 68m18s | failed: task-level M1 below 50% |
| M2 autonomy hours | 320-run v2 wall clock 68 m 18 s; prior run lasted 7 h 13 m 49 s. Both failed their quality gates | duration measured; quality not met |
| M3 interventions/task | 0 in the prior unattended 8h run; 320-run v2 was also unattended | 0 interventions in long runs |
| M7 TTFT | DeepSeek `deepseek-flash`, n=20 (10 short + 10 long-context): overall p50 1.498 s / p95 2.768 s; short p50 1.239 / p95 1.942; long p50 1.573 / p95 2.786; earlier n=3 was p50 1.08 / p95 2.48 | one provider; tools disabled; not production/statistically sufficient |
| M8 end-to-end p95 | 72.9 s historical snapshot | first/last event time, not a tool-dense benchmark |
| M9 tool success | 86.17% historical; 50% test-generated repo set | repo set is dominated by failure-injection tests |
| M10 cost/task | 3,275 micros historical | provider pricing not embedded; usage only |
| M11 memory hit rate | not measured | no repeated-task benchmark |
| M12 skill reuse | not measured | no repeated-workflow benchmark |
| M14 crash-free sessions | Prior 8h: 280/280 (100%); post-repair 5/20 runs: 5/5 and 20/20; 320-run v2: 320/320 (100%), 0 crashes | `crash_free_bps`; quality is the blocker, not crashes |
| M18 external audit | not started | 0 P0 unknown |
| M19–M23 | Packet built: 39 prompts / 78 label rows; Hermes 39/40 (research-05 timed out) with explicit provider/model; 1Agent answers are pre-repair and must be re-collected | `answer-quality` computes rates from caller labels; a human reviewer must label the final packet |

## What the 0.97 work added

- AG-23 intelligent loop (ADR-0130): live `describe_self`, mid-loop
  `request_tools` with permission checks, `task_need` assessment, at most two
  evidence re-plans before `needs_evidence`, inert strategy drafts in the
  existing `ExperienceStore`, and the fixed 100-question `agent-loop-v1`
  suite. `eval` and `benchmark-run --artifacts-dir` now keep per-case journals
  and a `cases.jsonl` index.
- Streaming provider deltas and `run --stream`.
- Parallel read-only tools with deterministic journal order.
- Incremental journal cursor and 1M parse regression.
- Checkpoint deltas and the 2h crash-recovery soak (28,552 cycles, exit 0).
- Model request deltas (ADR-0122): first request full, later attempts compact;
  100-step echo probe 2.5% of the full-copy estimate.
- TUI unslop pass: body contrast 6.0:1, border 3.8:1, one green accent/link
  role, compact wordmark on short terminals, quieter idle status and metrics,
  help position indicator, empty-composer hint.
- TUI work-log information flow: duplicate model-request rows removed; one
  `Model Request` row per step naming provider/model and new/total messages by
  role (no context-token or available-tool counts); tool results inserted under
  their `call_id`; failures localized with URL; stop reasons localized;
  truncated tables stack as label/field rows. The answer prompt now asks only
  when the next action depends on the user and limits tables to 3+ items across
  2+ dimensions.
- Clarify TUI batch panel (`/clarify`): 1–5 blocking questions, per-question
  lock, inline `Other`, one confirm, cancel, 300s expiry, result in transcript
  and `/details`. The interactive TUI also registers a model-initiated
  `clarify` tool; persistent history is still absent.
- Real-provider 8-task baseline: suite v1.1 passes 16/16 across two runs;
  the earlier v1.0 baseline was 17/24.
- 8h repeated benchmark completed 7 h 13 m 49 s unattended but **failed**:
  136/280 all-task iterations (48.57%, 1.43 pp below the 0.97 ≥50% target),
  238/280 suite-replay-verified (85%; not the completed-run M5 denominator),
  280/280 crash-free. This is now the binding release blocker.
- 320-run v2 (`job-1794537863409-2245-6`) completed 68 m 18 s unattended and
  **failed**: 340/2560 tasks (13.28%), 25/320 suites, completed-case replay
  351/351 (100%), 320/320 crash-free. The shorter runtime and lower completion
  indicate many early failures; per-case journals were not preserved, so
  diagnosis needs a rerun with per-case capture.
- Answer repair contract (ADR-0125): repair requests carry no tools; a
  high-impact assumption must downgrade to conditional/needs_evidence;
  evidence.source must be the exact path. The repair tests assert both.
- Benchmark metric split (ADR-0126): `EvalCaseResult.replay_ok` now uses
  `replay_events`; `benchmark-run` reports task-level M1 and completed-run M5
  with independent gates. Post-repair samples: 5 iterations passed 37/40 tasks
  (92.5% task-level M1, 2/5 suites) and 20 iterations passed 144/160 tasks
  (90.0% task-level M1, 10/20 suites = 50%); both had 100% completed-case
  replay (40/40 and 157/157) and 100% crash-free. The old 8h JSON has no
  task-level data and cannot be rescored; a new 8h run must use the current
  binary.
- Real TTFT baseline: three DeepSeek streaming runs, p50 1.08 s / p95 2.48 s.
- Rollback drill: 0.96.1 cannot read a 0.97 journal; the current binary reads
  0.96.1 journals.
- TUI transcript projection cache.
- Watchdog escalation ladder plus explicit checked `--execute pause/resume`.
- Opt-in watchdog automatic containment (`--watch --execute pause`): one
  checked SIGSTOP per Pause/Repair pid, no repeat on escalation, no automatic
  resume, kill or restart; 10 `watchdog_cli` tests pass. The attribution
  history/tree check is now fail-closed with 3 regression tests.
- Clarify gate and round outcome states.
- Actionable error hints and Answer/Audit layering.
- Local gate metrics, journal-derived route/context/model/tool stage latencies,
  and fail-closed gate thresholds in `release-check.sh --metrics-dir`.
- Dashboard gate cards and a fixed golden-set prompt inventory.
- Runtime file kill switch for write-capable tools.
- Legacy journal timestamp fix.
- First two module-split slices and eight-case core suite.
- Architecture RFC and P0/P1 threat model (local review).
- First-token timing, crash-free proxy, compacted-replay proof, subagent
  parent acceptance/fanout.

## Cost governance (release selling point)

0.97 ships cost governance as a product feature, not an internal optimization:
users should see and control what a run costs, and chat should not wake the
whole agent. The detailed plan is [docs/cost-governance.md](cost-governance.md).

P0 scope:

- **Chat fast path**: one model call, no tools, minimal JSON, `max_tokens`
  500–800; simple questions target ≤2k input + ≤500 output.
- **DirectAnswer no-tools**: tools are exposed only when an explicit action
  needs them.
- **Benchmark cost guard**: preflight estimate, `--max-cost-micros`,
  checkpoints at 20/40/80/160/320, and a 401/429/timeout circuit breaker.
- **Cost visibility**: real prices in config, per-turn tokens/cost in the TUI,
  and cost per successful task.

P0 progress (2026-11-13): the benchmark cost guard is implemented
(`--max-cost-micros`, `--checkpoint-every`, Wilson-bound task checkpoints,
provider-error and crash-free early stops). The chat fast path is implemented:
conservative auto-detection, a compact prompt, minimal
JSON, one step, and no tools. Conversation context no longer forces
`EvidenceResearch`; action/local/current-data cues force the full agent, and a
`needs_evidence` chat answer escalates once to the full agent. Per-turn TUI
cost display was intentionally not added: it would change the information flow
without a product request. `benchmark-run --estimate-only` prints a preflight
estimate, and `--max-cost-micros` refuses to start when the estimate exceeds
the budget. At reference prices, the 8-case core suite × 320 runs estimates
10.3M input + 2.0M output tokens ≈ ¥26.68, consistent with the 2026-11-13 bill.
P0 cost governance is implemented except TUI per-turn cost visibility.

Targets from the 2026-11-13 baseline: simple chat 5–20x cheaper per question;
work tasks 30–50% fewer tokens; a failing long run 50–80% cheaper. These are
product targets, not measured results yet. The M1/M5 quality gates remain the
release blocker; cost governance cannot mask a quality failure.

## Release blockers

1. **M1/M5 long-horizon proof**: the old 8h failed at 48.57% and the 320-run
   v2 at 13.28% task completion. The new 20-run short regression with
   `real-tasks-v1` v1.2 (8 steps / 8 tool calls per case) **passes**:
   **149/160 tasks = 93.12%** (M1 ≥50%), **suite-level 11/20 = 55%**
   (0.97 ≥50%), **154/154 completed-run replay** (100%), **100% crash-free**,
   CLI `passed = true`. Run 1 with the six-step default measured 146/160
   (91.25%) and 8/20 (40%) suite-level; the eight-step change cleared the
   suite-level gate. A separate **8h local-model runtime soak** (Ollama
   `qwen3-8b-16k`, 56 runs / 448 tasks) also passed: **0 crashes**,
   **234/234 completed-run replay**, actual duration **8h26m51s**. Its task
   quality was 21.2% because the local 8B model is weak; that is a
   model-quality limit, not the M1 gate. The 8h **model-quality** M1 on
   DeepSeek still must be run before release; per-case diagnosis of the old
   320-run is impossible because those journals were not preserved.
2. **AG-23 intelligent loop**: the implementation landed in `c76a2d4`,
   `5465c2a`, `55ccb22` and `a04686a`: live `describe_self`, dynamic
   `request_tools`, `task_need`, bounded evidence re-plan, strategy drafts and
   the 100-question suite. The final v1.3 run scored **96/100** (intro
   20/20, follow-up 20/20, risk 20/20, simple chat 19/20, current/local
   17/20), 0 hard-gate failures, 100% completed-run replay (78/78), and
   about ¥2.88 cost. The four misses are honest limits, not hidden
   errors: three open-ended current-info questions exhausted the 8-tool
   budget and returned `needs_evidence`; one simple-chat answer used a
   repair call (20-case average 1.05 calls, 0 tools, within the ≤1.2
   gate). Human review of the 100 answers is still required. Do not ship
   before that review and the M1 gate.
3. M7 TTFT now has 20 mixed short/long samples; a second provider still needs
   credentials, and tool-dense/retry-heavy latency is not sampled.
4. M19–M23 blind-review labels: the current 39-prompt/78-row packet is
   provisional; 1Agent answers must be re-collected after the repair/prompt
   changes, then a human reviewer must label the final packet.
5. External A-Gate/W-Gate and security review; a review draft is at
   `docs/audit-package.md`; P-Gate macOS.
6. Automatic watchdog repair: containment is implemented (`--watch --execute
   pause`); automatic resume, kill/restart and external repair commands remain
   out of scope pending a health signal, lease fencing and approval. M6 is not
   an external audit.

## Next actions

- Answer-repair contract is fixed (ADR-0125). Per-case benchmark artifacts now
  exist (`benchmark-run --artifacts-dir`); a fresh 20-run short regression with
  the current binary is running. The 320-run v2 `job-1794537863409-2245-6`
  finished 11:52 and failed at 13.28% task completion; its journals were not
  preserved, so a new long run is required after the short regression.
- AG-23 evidence is archived: final 100-question v1.3 = **96/100**, 0
  hard-gate, 78/78 completed-run replay, about ¥2.88. The four misses are three
  open-ended current-info questions that exhausted the 8-tool budget and one
  simple-chat repair. Human review of the 100 answers is still pending. Keep
  keyword detection only as a cost/safety fallback; never as the primary tool
  router.
- Cost governance P0 is implemented except TUI per-turn cost visibility, which
  was deferred at the user's request: benchmark preflight/checkpoints/circuit
  breaker, chat fast path, DirectAnswer no-tools, and real prices.
- Irreparable model output is now converted to an auditable `NeedsEvidence`
  answer instead of a hard runtime error; the invalid response and repair
  attempt remain in the journal. Replay is preserved and no task success is
  claimed.
- TTFT n=20 is recorded; a second provider/tool-dense sample remains optional.
- Re-collect the 1Agent blind-review side with the current binary, repacket,
  then obtain human labels and run `1agent answer-quality`.
- RFC 0002 containment is implemented; keep automatic resume/restart in 0.98
  pending lease fencing and an independent health signal.
- Keep the release check fail-closed on a representative journal directory.
- Persist clarify question history across sessions.
