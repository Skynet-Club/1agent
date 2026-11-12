# 0.97 Status and Metric Review (2026-11-12, 16:16 CST)

- Status: **local review; 0.97 is not release-ready**
- Latest commit at review: `87c51ad`
- Full suite: 1230 passed / 0 failed / 13 ignored
- Claims: 219 (197 verified / 14 partial / 8 no evidence)
- Local release: Linux and Windows GNU binaries are rebuilt per slice; the
  Windows `.exe` is linked, not run on Windows

This page is a measurement review, not a release announcement. A metric marked
“not measured” is not zero; it means the evidence does not exist yet.

## Hard gates

| Gate | Metric | Current evidence | Status |
|---|---|---|---|
| M4 | Crash recovery ≥95% | Controlled resume sample 1/1 (100%); 2h crash-injection soak started 15:49 CST and is still running; no aggregate from real crashes | **Open** |
| M5 | Replay 100% | 100% structural replay on completed journals in both local snapshots | **Local pass** |
| M6 | Zero security incidents | No P0 in the local red-team/security tests; 5 denied observations in historical journals | **Not externally verified** |
| M13 | Audit completeness 100% | 100% under `AgentAnswer::validate` on local journals | **Local pass** |
| M15 | TUI frame p95 ≤8 ms | Release 1.26 ms on a 200-entry transcript, `TestBackend` | **Local pass** |
| M16 | 1M journal incremental p95 ≤50 ms | 4.7 MB journal: full 31.8 ms, incremental p95 9.4–16.1 µs | **Local pass** |
| M17 | Compacted run replay 100% | Forced-compaction runtime fixture replays, audits and reports 100% | **Local pass** |

## Targets and baselines

| Metric | Current | Note |
|---|---|---|
| M1 long-horizon success | not measured | no real 8h benchmark |
| M2 autonomy hours | not measured | only short deterministic runs |
| M3 interventions/task | not measured | no real 8h benchmark |
| M7 TTFT | instrumented, no real-provider samples | `StreamFirstToken` on `--stream` runs; metrics prints p50/p95 |
| M8 end-to-end p95 | 72.9 s historical snapshot | first/last event time, not a tool-dense benchmark |
| M9 tool success | 86.17% historical; 50% test-generated repo set | repo set is dominated by failure-injection tests |
| M10 cost/task | 3,275 micros historical | provider pricing not embedded; usage only |
| M11 memory hit rate | not measured | no repeated-task benchmark |
| M12 skill reuse | not measured | no repeated-workflow benchmark |
| M14 crash-free sessions | proxy reported | `crash_free_bps` = runs with a terminal stop / runs |
| M18 external audit | not started | 0 P0 unknown |
| M19–M23 | board plus a fixed 40-prompt golden set; no labels | `answer-quality` computes rates from caller labels; blind review is external |

## What the 0.97 work added

- Streaming provider deltas and `run --stream`.
- Parallel read-only tools with deterministic journal order.
- Incremental journal cursor and 1M parse regression.
- Checkpoint deltas and crash-recovery soak (2h run pending).
- TUI transcript projection cache.
- Watchdog escalation ladder plus explicit checked `--execute pause/resume`.
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

1. 2h crash-recovery soak result (M4).
2. Real eight-task benchmark and M1/M2/M3 measurements.
3. M7 real-provider TTFT samples; M17 is now locally proven.
4. M19–M23 blind-review labels; the prompt inventory exists, labels do not.
5. External A-Gate/W-Gate and security review; P-Gate macOS.
6. Automatic watchdog repair remains manual; pause/resume is explicit and
   checked. M6 is not an external audit.
7. `ModelRequested` remains a full per-step journal copy.

## Next actions

- Finish the 2h soak and record its result.
- Add a real-task suite once provider credentials and reviewed tasks are
  available; do not relabel contract fixtures as model quality.
- Collect blind-review labels for the fixed golden set.
- Add first-stream-chunk timing to journals for M7 (instrumentation exists; real
  provider samples do not).
- Keep the release check fail-closed on a representative journal directory.
