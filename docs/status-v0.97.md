# 0.97 Status and Metric Review (2026-11-13, 09:55 CST)

- Status: **local review; 0.97 is not release-ready — the 8h gate failed**
- Latest feature commit: `f670f13` (TUI request-row refinement), after
  `46864c1` (route/role work log), `68db238` (work-log pairing), `a2ad66c`
  (repair contract and M1/M5 metrics) and `41cb333` (watchdog containment);
  earlier evidence commits `bbecf78` (M4 soak), `7e6306f` (Clarify panel),
  `16ce37f` (TUI unslop), `ace343e` (model request deltas)
- Full suite: 1269 passed / 0 failed / 13 ignored
- Claims: 226 (202 verified / 16 partial / 8 no evidence); the live terminal
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
| M1 long-horizon success | **First 8h failed**: 136/280 suites (48.57%). Post-repair 20-run: **144/160 tasks (90.0%)**, 10/20 suites (50%), M5 157/157. A new 320-run 8h (`job-1794537863409-2245-6`) is running with task M1 ≥50% and M5 100% | first run not met; post-repair samples pass; new 8h pending |
| M2 autonomy hours | New 8h in progress; prior run lasted 7 h 13 m 49 s but failed its quality gate | new result pending |
| M3 interventions/task | 0 in the prior unattended 8h run | new run is unattended too |
| M7 TTFT | DeepSeek `deepseek-flash`, n=20 (10 short + 10 long-context): overall p50 1.498 s / p95 2.768 s; short p50 1.239 / p95 1.942; long p50 1.573 / p95 2.786; earlier n=3 was p50 1.08 / p95 2.48 | one provider; tools disabled; not production/statistically sufficient |
| M8 end-to-end p95 | 72.9 s historical snapshot | first/last event time, not a tool-dense benchmark |
| M9 tool success | 86.17% historical; 50% test-generated repo set | repo set is dominated by failure-injection tests |
| M10 cost/task | 3,275 micros historical | provider pricing not embedded; usage only |
| M11 memory hit rate | not measured | no repeated-task benchmark |
| M12 skill reuse | not measured | no repeated-workflow benchmark |
| M14 crash-free sessions | Prior 8h: 280/280 (100%); post-repair 5/20 runs: 5/5 and 20/20 | `crash_free_bps`; new 8h pending |
| M18 external audit | not started | 0 P0 unknown |
| M19–M23 | Packet built: 39 prompts / 78 label rows; Hermes 39/40 (research-05 timed out) with explicit provider/model; 1Agent answers are pre-repair and must be re-collected | `answer-quality` computes rates from caller labels; a human reviewer must label the final packet |

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

## Release blockers

1. **M1/M5 long-horizon proof**: the first 8h failed at 48.57% all-suite
   completion. After the repair contract fix, 20 short iterations pass 90% of
   tasks (M1) and 100% of completed-case replay (M5); a new 320-run 8h is
   running. Do not release until it passes with the current binary.
2. M7 TTFT now has 20 mixed short/long samples; a second provider still needs
   credentials, and tool-dense/retry-heavy latency is not sampled.
3. M19–M23 blind-review labels: the current 39-prompt/78-row packet is
   provisional; 1Agent answers must be re-collected after the repair/prompt
   changes, then a human reviewer must label the final packet.
4. External A-Gate/W-Gate and security review; a review draft is at
   `docs/audit-package.md`; P-Gate macOS.
5. Automatic watchdog repair: containment is implemented (`--watch --execute
   pause`); automatic resume, kill/restart and external repair commands remain
   out of scope pending a health signal, lease fencing and approval. M6 is not
   an external audit.

## Next actions

- Answer-repair contract is fixed (ADR-0125). The 20-iteration sample passes
  task-level M1 (90%) and completed-run M5 (100%); the new 320-run 8h
  `job-1794537863409-2245-6` is running. Update M1/M2/M5 when it finishes.
- Decide whether an irreparable model output should become a valid
  `NeedsEvidence` answer (auditable journal) instead of a hard runtime error;
  this would improve replay without claiming task success.
- TTFT n=20 is recorded; a second provider/tool-dense sample remains optional.
- Re-collect the 1Agent blind-review side with the current binary, repacket,
  then obtain human labels and run `1agent answer-quality`.
- RFC 0002 containment is implemented; keep automatic resume/restart in 0.98
  pending lease fencing and an independent health signal.
- Keep the release check fail-closed on a representative journal directory.
- Persist clarify question history across sessions.
