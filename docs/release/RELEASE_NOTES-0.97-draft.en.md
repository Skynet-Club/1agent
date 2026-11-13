# Release Notes — 0.97.0 Reliable Loop (Draft)

[简体中文](RELEASE_NOTES-0.97-draft.md) · [English](RELEASE_NOTES-0.97-draft.en.md)

Date: 2026-11-13

Status: **release-candidate draft, not published; 8h gate failed**

Pending: reliability fixes and a new 8h run, broader TTFT sample, human
blind-review labels, external A-Gate/W-Gate/security review, macOS P-Gate,
release package.

> This draft is not a release announcement. A metric marked “pending” is not
> zero; the evidence does not exist yet.

## What the product is

1Agent is a local-first, evidence-accountable super-agent runtime. It makes
the task, evidence, authorization, execution, acceptance and audit chain
reconstructable from a journal. It plans and executes under explicit
permissions; without an execution substrate it is analysis/planning only.

0.97 remains on the R1 pre-release line. It is not production-stable and not an
accepted Super-Agent.

## Release positioning

0.97.0 “Reliable Loop” is about closing the long-horizon reliability loop, not
adding more surface area:

- streaming output and first-token timing;
- parallel read-only tools with deterministic journal order;
- incremental journal reads and a 1M-token parse regression;
- checkpoint and model-request deltas that shrink journals;
- crash recovery/resume plus a 2h injection soak;
- a Clarify batch panel and a model-initiated clarify tool;
- TUI readability, recovery hints and transcript caching;
- observable gates, a real-task suite, rollback and audit material.

Computer use, automatic restart, multi-agent organization, external audit and
the cross-platform P-Gate remain 0.98–1.0 work.

## Highlights

### 1. Reliable loop and journal

- `--stream` records `StreamFirstToken` for the first provider attempt;
  `metrics` reports TTFT p50/p95. Non-streaming runs have no sample.
- Read-only tools may run in parallel, but tool calls/observations keep a
  deterministic journal order and pair-wise audit.
- Incremental journal reads parse only complete lines after the cursor and
  retain an unterminated tail. A 4.7 MB journal parses fully in 31.8 ms;
  incremental p95 is 9.4–16.1 µs.
- Checkpoint deltas store the first checkpoint in full and later only new
  messages. A 100-step checkpoint probe is 115 KB versus a ~2.99 MB full-copy
  estimate.
- Model-request deltas (ADR-0122) store the first request in full and later
  `ModelRequestDelta` values after the unchanged prefix. The 100-step echo
  probe is 74.9 KB versus a ~3.03 MB full-copy estimate (2.5%).
  `ModelRequestChain` reconstructs requests; audit rejects orphan deltas and
  prefix overflow.
- A 2h crash-injection soak completed 28,552 cycles in 7,200.20s with exit 0.
  Completed side effects did not repeat; mid-call and checkpoint-less journals
  failed closed.
- Rollback drill: the current binary reads 0.96.1 journals; the published
  0.96.1 binary cannot read 0.97 journals with `checkpoint_delta` or
  `model_request_delta`.

### 2. Clarify: ask before acting

- `/clarify` is a 1–5 question blocking batch panel with per-question locks,
  inline `Other`, one confirm, cancel and a 300s timeout.
- Round outcomes: `submitted|cancelled|timed_out|undelivered`; answer states:
  `answered|skipped|unanswered`. An unanswered question does not invalidate a
  default answer.
- The interactive TUI registers a model-initiated `clarify` tool; line and
  headless runs do not. A real PTY run exercised model request → panel → tool
  observation → final answer.
- Limitation: question history is not persisted across sessions; mouse and IME
  are not covered.

### 3. TUI readability and feedback

- Unslop audit: body contrast 6.0:1, border 3.8:1; one green accent/link role;
  compact wordmark on short terminals; quieter idle status and metrics; a help
  position indicator; an empty-composer hint.
- One `Model Request` row per step naming provider/model and the new/total
  message breakdown by role, with no context-token or available-tool counts;
  tool results stay under their call id; common failures are localized with the
  URL; stop reasons are localized; a table that would truncate renders as
  stacked rows.
- Full-screen Clarify panel, recovery hints and a transcript projection cache
  (release frame p95 1.26 ms on a 200-entry transcript).
- Line and interactive modes share the same recovery hint; failures append
  `Hint: ...` (Chinese TUI: `提示：...`).

### 4. Observability, evaluation and release gates

- `metrics` projects completion, recovery, structural replay, compacted
  replay, tool success, audit completeness, denied observations, tokens/cost,
  run latency and TTFT from journals.
- `gate-check.sh` fails closed on a missing sample; the dashboard adds gate
  cards.
- Real-task suite v1.1: DeepSeek `deepseek-flash` passed 16/16 across two short
  runs; the earlier v1.0 baseline was 17/24 (70.83%). The 8h repeated benchmark
  finished below the 0.97 target: 136/280 all-task iterations (48.57%, 1.43
  points below the ≥50% target), 238/280 suite-replay-verified (85%; not the
  completed-run M5 denominator), 280/280 crash-free.
- Real TTFT baseline: three short streaming runs, p50 1.08 s / p95 2.48 s. The
  expanded sample collected 17/20 (p50 1.31 s / p95 2.26 s); three runs lacked
  a content token because the first response was a tool call, so a rerun is
  needed.
- `release-check.sh` exited 0 on clean tree `39ea96e` (hash rewritten from
  `509061c` when tool bylines were removed): fmt, clippy, 1261 passed
  / 0 failed / 13 ignored, HTTP e2e, CI status 0, claims evidence execution and
  TUI PTY interaction. The metrics gate was skipped without
  `--metrics-dir`.

### 5. Security and operations

- Runtime kill switch: when `~/.1a/KILL` exists, all write-capable tools fail
  closed; read-only tools remain available.
- Watchdog escalation ladder and checked `--execute pause|resume`.
- 0.97 adds opt-in automatic containment: `--watch --execute pause` sends one
  checked SIGSTOP per Pause/Repair pid and does not re-signal on
  Pause-to-Repair escalation. It never auto-resumes, kills, restarts or clears
  locks. Unix only; the cmdline-check/signal TOCTOU remains.
- Threat model, audit-package draft, and claims at 200 ✅ / 16 ⚠️ / 8 ❌.
  External audit has not started.

## Verification summary

| Item | Result | Status |
|---|---|---|
| Full tests (current) | 1265 passed / 0 failed / 13 ignored | local pass |
| fmt / clippy | clean | local pass |
| release-check | exit 0 (metrics gate skipped) | local pass |
| 2h crash-injection soak | 28,552 cycles / 7,200.20s / exit 0 | local pass |
| Structural replay | 100% on completed journals | local pass |
| Audit completeness | 100% under `AgentAnswer::validate` | local pass |
| Real-task v1.1 | 16/16 across two short runs | local pass, not 8h |
| 8h M1/M2/M3 | 136/280 completed (48.57%; 0.97 target ≥50%), 238/280 suite replay (85%; M5 needs completed-run calc), 280/280 crash-free | **target missed** |
| M7 TTFT | n=20 (10 short + 10 long-context) p50 1.50 s / p95 2.77 s; short p50 1.24 / p95 1.94; long p50 1.57 / p95 2.79 | one provider; tools disabled |
| M19–M23 blind review | packet pipeline ready | human labels pending |
| External audit / macOS P-Gate | not started | pending |

## Known limits

- 0.97 is not production-stable; interfaces, configuration and journal formats
  may still change.
- A 0.97 journal cannot be downgraded to 0.96.1 for reading; back up before
  upgrading.
- Old binaries cannot read journals with `checkpoint_delta` or
  `model_request_delta`.
- Automatic restart, external repair commands and automatic resume are not in
  0.97.
- The watchdog only signals Unix processes; its intervention log has no
  rotation.
- `cost_micros` is meaningful only when provider pricing is configured; the
  current DeepSeek config does not embed prices.
- M1/M2/M3, the broader M7 sample, M19–M23 human labels, external
  A-Gate/W-Gate/security review and macOS P-Gate remain open. Do not claim
  Super-Agent or production stability from this draft.

## Upgrade and rollback

1. Back up the runtime home (`~/.1a`) and journal directories.
2. The 0.97 binary reads older journals; the reverse is not guaranteed.
3. See `docs/rollback-v0.97.md` for rollback points and steps.
4. The `~/.1a/KILL` kill switch blocks write tools; it does not kill a process,
   restart a daemon or clear locks.

## Release assets (not generated)

- `1agent-0.97.0-linux-x86_64.tar.gz` — Linux x86_64 package;
- `SHA256SUMS` — archive and provenance checksums;
- `linux-x86_64.json` — Linux build provenance.

The assets are not generated; this file is not a release announcement.
