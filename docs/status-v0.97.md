# 0.97 Status and Metric Review (2026-11-13, 09:55 CST)

- Status: **local review; 0.97 is not release-ready — the 8h gate failed**
- Latest feature commit: `41cb333` (watchdog containment; hash rewritten from
  `e1291b1` when tool bylines were removed), based on
  `825af12`; earlier evidence commits `bbecf78` (M4 soak), `7e6306f` (Clarify
  panel), `16ce37f` (TUI unslop), `ace343e` (model request deltas)
- Full suite: 1264 passed / 0 failed / 13 ignored
- Claims: 224 (200 verified / 16 partial / 8 no evidence); the live terminal
  claim is partial because the local session daemon is at its 128-session limit
  (`session_limit`), not because the terminal tool failed
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
| M1 long-horizon success | **8h failed**: 280 iterations, 136 completed (48.57%), replay 238/280 (85.00%), crash-free 280/280 (100%). The 0.97 target is ≥50%, so M1 is 1.43 pp short; the run used the 1.0 70% completion gate. Diagnostic sample: 21/24 tasks, 1/3 all-8 suites; failures are model-contract violations | not met; one provider/model; see `docs/evidence/long-horizon-benchmark-2026-11-12.md` |
| M2 autonomy hours | 7 h 13 m 49 s unattended (`job-1794496736687-2245-2`), but the run failed its quality gate | duration recorded; not counted as an M2 pass |
| M3 interventions/task | 0 in the unattended 8h run | interventions were not possible in the scheduled job |
| M7 TTFT | DeepSeek `deepseek-flash`: earlier n=3 p50 1.08 s / p95 2.48 s; new run collected 17/20 samples p50 1.309 s / p95 2.261 s (3 missing because the first response was a tool call, not content) | one provider; rerun with a direct-answer contract needed for n=20 |
| M8 end-to-end p95 | 72.9 s historical snapshot | first/last event time, not a tool-dense benchmark |
| M9 tool success | 86.17% historical; 50% test-generated repo set | repo set is dominated by failure-injection tests |
| M10 cost/task | 3,275 micros historical | provider pricing not embedded; usage only |
| M11 memory hit rate | not measured | no repeated-task benchmark |
| M12 skill reuse | not measured | no repeated-workflow benchmark |
| M14 crash-free sessions | 280/280 (100%) in the 8h run | `crash_free_bps` = runs with a terminal stop / runs; failed model contracts are not crashes |
| M18 external audit | not started | 0 P0 unknown |
| M19–M23 | board plus a fixed 40-prompt golden set; 1Agent answers collected; Hermes safe-mode needs explicit `--provider`/`--model` (script fixed), so the first packet job failed and Hermes collection is rerunning | `answer-quality` computes rates from caller labels; a human reviewer must label the packet |

## What the 0.97 work added

- Streaming provider deltas and `run --stream`.
- Parallel read-only tools with deterministic journal order.
- Incremental journal cursor and 1M parse regression.
- Checkpoint deltas and the 2h crash-recovery soak (28,552 cycles, exit 0).
- Model request deltas (ADR-0122): first request full, later attempts compact;
  100-step echo probe 2.5% of the full-copy estimate.
- TUI unslop pass: body contrast 6.0:1, border 3.8:1, one green accent/link
  role, compact wordmark on short terminals, quieter idle status and metrics,
  help position indicator, empty-composer hint.
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
- Answer repair contract (ADR-0125): repair requests carry no tools; a
  high-impact assumption must downgrade to conditional/needs_evidence;
  evidence.source must be the exact path. The repair tests assert both.
- Benchmark metric split (ADR-0126): `EvalCaseResult.replay_ok` now uses
  `replay_events`; `benchmark-run` reports task-level M1 and completed-run M5
  with independent gates. A 5-iteration validation run uses M1 ≥50% and M5
  100%; the old 8h JSON has no task-level data and cannot be rescored.
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

## Release blockers

1. **M1/M5 reliability**: the 8h run reached 48.57% all-suite completion,
   1.43 pp below the 0.97 ≥50% target (and far below the 1.0 70% gate). The
   benchmark's 85% replay counts invalid-output suites; M5 is defined over
   completed runs and still needs its own calculation. Failures are
   model-contract violations (repair returned tool calls, high-impact
   assumption, Conditional vs Completed, prose evidence source). Fix the
   repair/output contract and rerun; do not lower the gate.
2. M7 broader TTFT samples: 17/20 collected; 3 lacked a first content token
   because the model called a tool. Rerun with a direct-answer contract. A
   second provider still needs credentials.
3. M19–M23 blind-review labels; the Hermes collector needed explicit
   provider/model and is rerunning; a human reviewer must label the packet.
4. External A-Gate/W-Gate and security review; a review draft is at
   `docs/audit-package.md`; P-Gate macOS.
5. Automatic watchdog repair: containment is implemented (`--watch --execute
   pause`); automatic resume, kill/restart and external repair commands remain
   out of scope pending a health signal, lease fencing and approval. M6 is not
   an external audit.

## Next actions

- Answer-repair contract is fixed (ADR-0125). A 5-iteration benchmark with
  task-level M1 ≥50% and completed-run M5 100% is running; use its per-task
  diagnostics before scheduling a larger run.
- Decide whether an irreparable model output should become a valid
  `NeedsEvidence` answer (auditable journal) instead of a hard runtime error;
  this would improve replay without claiming task success.
- Rerun the 20-sample TTFT set with a direct-answer/no-tool contract.
- Finish the Hermes blind-review collection, build the packet, then obtain 80
  human labels; run `1agent answer-quality`.
- RFC 0002 containment is implemented; keep automatic resume/restart in 0.98
  pending lease fencing and an independent health signal.
- Keep the release check fail-closed on a representative journal directory.
- Persist clarify question history across sessions.
