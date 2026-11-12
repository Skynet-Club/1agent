# 0.97 Status and Metric Review (2026-11-13, 00:30 CST)

- Status: **local review; 0.97 is not release-ready**
- Latest feature commit: `e1291b1` (watchdog containment), based on
  `825af12`; earlier evidence commits `bbecf78` (M4 soak), `7e6306f` (Clarify
  panel), `16ce37f` (TUI unslop), `ace343e` (model request deltas)
- Full suite: 1261 passed / 0 failed / 13 ignored
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
| M1 long-horizon success | current suite v1.1: 16/16 (100%) across two short runs; earlier v1.0 baseline 17/24 (70.83%); 8h run `job-1794496736687-2245-2` in progress | not an 8h result yet; 8 tasks, one provider/model |
| M2 autonomy hours | 8h run in progress (`job-1794496736687-2245-2`) | unattended; result appended when the job finishes |
| M3 interventions/task | 0 in the unattended 8h run so far | interventions are not possible in the scheduled job |
| M7 TTFT | DeepSeek `deepseek-flash`, n=3 short streaming runs: p50 1.08 s, p95 2.48 s (min 0.72, max 2.48); broader n=20 (10 short + 10 long-context) scheduled as `job-1794499887386-2245-5` after the 8h run | `StreamFirstToken` + `metrics`; one provider; broader sample not collected yet |
| M8 end-to-end p95 | 72.9 s historical snapshot | first/last event time, not a tool-dense benchmark |
| M9 tool success | 86.17% historical; 50% test-generated repo set | repo set is dominated by failure-injection tests |
| M10 cost/task | 3,275 micros historical | provider pricing not embedded; usage only |
| M11 memory hit rate | not measured | no repeated-task benchmark |
| M12 skill reuse | not measured | no repeated-workflow benchmark |
| M14 crash-free sessions | proxy reported | `crash_free_bps` = runs with a terminal stop / runs |
| M18 external audit | not started | 0 P0 unknown |
| M19–M23 | board plus a fixed 40-prompt golden set; blind-review packet pipeline ready; A/B collection scheduled after the 8h benchmark (`job-1794497599059-2245-3`) | `answer-quality` computes rates from caller labels; a human reviewer must label the packet |

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
  the earlier v1.0 baseline was 17/24. 8h M1/M2/M3 remain unmeasured.
- Real TTFT baseline: three DeepSeek streaming runs, p50 1.08 s / p95 2.48 s.
- Rollback drill: 0.96.1 cannot read a 0.97 journal; the current binary reads
  0.96.1 journals.
- TUI transcript projection cache.
- Watchdog escalation ladder plus explicit checked `--execute pause/resume`.
- Opt-in watchdog automatic containment (`--watch --execute pause`): one
  checked SIGSTOP per Pause/Repair pid, no repeat on escalation, no automatic
  resume, kill or restart; 10 `watchdog_cli` tests pass.
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

1. M1 8h long-horizon, M2 autonomy hours and M3 interventions; the current
   8-task suite v1.1 passes 16/16 but is a short sample.
2. M7 broader TTFT samples (long-task); a 3-sample baseline exists and a
   scheduled n=20 short/long run waits for the 8h benchmark. A second provider
   still needs credentials.
3. M19–M23 blind-review labels; the packet pipeline is ready and collection is
   scheduled, but a human reviewer must label it.
4. External A-Gate/W-Gate and security review; a review draft is at
   `docs/audit-package.md`; P-Gate macOS.
5. Automatic watchdog repair: containment is implemented (`--watch --execute
   pause`); automatic resume, kill/restart and external repair commands remain
   out of scope pending a health signal, lease fencing and approval. M6 is not
   an external audit.

## Next actions

- M4 soak raw output is recorded; collect a real-crash aggregate if the gate
  requires one.
- Human-review the 8-task suite v1.1 (16/16 automated); do not relabel
  contract fixtures as model quality.
- Label the blind-review packet and run `1agent answer-quality`; the packet
  has 40 prompts and 80 label rows.
- RFC 0002 containment is implemented; keep automatic resume/restart in 0.98
  pending lease fencing and an independent health signal.
- Run an 8h benchmark for M1/M2/M3 and broaden TTFT samples beyond one provider
  and short prompts.
- Keep the release check fail-closed on a representative journal directory.
- Persist clarify question history across sessions.
