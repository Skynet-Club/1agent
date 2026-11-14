# Release Notes — 0.97.0 Reliable Loop (Draft)

[简体中文](RELEASE_NOTES-0.97-draft.md) · [English](RELEASE_NOTES-0.97-draft.en.md)

Date: 2026-11-14

Status: **release-candidate draft, not published; local gates and the Linux
package are being finalized**

Pending: third-party A-Gate/W-Gate/security certification, a human blind
review (the archived answer-quality labels are AI/bulk-written), macOS P-Gate,
a rebuilt clean public history, and the final package confirmation.

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
- Real-task 1h gate (product scope ≤1h): DeepSeek `deepseek-flash`, 30 runs /
  240 tasks, M1 **233/240 = 97.08%**, suite **23/30 = 76.66%**, completed
  replay **239/239 = 100%**, crash-free 100%, 44m16s, about ¥11.63. One
  provider error lost a case record, so the historical suite audit+replay gate
  is 29/30 and the CLI prints `passed=false`; the task-level gate passed and
  the discrepancy is archived.
- 8h local runtime soak: Ollama `qwen3-8b-16k`, 56 runs / 448 tasks, 0 crashes,
  completed replay 234/234, 8h26m51s. Local model task quality was only 21.2%,
  so this proves runtime stability, not M1 model quality; no 8h DeepSeek
  quality claim is made.
- 100-question AG-23 loop: **96/100**; intro 20/20, follow-up 20/20,
  high-risk 20/20, simple chat 19/20, current/local 17/20; completed replay
  78/78.
- Real TTFT baseline: n=20 (10 short + 10 long-context), p50 1.50 s / p95
  2.77 s; short p50 1.24 / p95 1.94, long p50 1.57 / p95 2.79; one provider,
  tools disabled.
- `release-check.sh` runs fmt, clippy, the full test suite, HTTP e2e, claims
  and TUI PTY on the current tree. `check-attribution.sh` is still red on local
  `main` because of two historical commits; a public release must be cut from
  rebuilt clean history. The metrics gate is skipped without
  `--metrics-dir`.

### 5. Security and operations

- Runtime kill switch: when `~/.1a/KILL` exists, all write-capable tools fail
  closed; read-only tools remain available.
- Watchdog escalation ladder and checked `--execute pause|resume`.
- 0.97 adds opt-in automatic containment: `--watch --execute pause` sends one
  checked SIGSTOP per Pause/Repair pid and does not re-signal on
  Pause-to-Repair escalation. It never auto-resumes, kills, restarts or clears
  locks. Unix only; the cmdline-check/signal TOCTOU remains.
- Threat model, audit package and an independent code-level review: no P0, five
  P1 classes fixed (approval outcome, subagent permission, MCP environment and
  annotations, resume replay, provider redirects); four P1 residual items
  (browser_read SSRF, run-journal hash chain, cross-builder reproducibility,
  historical attribution) and the P2 list are in `docs/audit-v0.97.md`.
  Claims are 233 (209 ✅ / 16 ⚠️ / 8 ❌); third-party audit/certification has
  not started.

## Verification summary

| Item | Result | Status |
|---|---|---|
| Full tests (current) | `cargo test --offline -j 2` green | local pass |
| fmt / clippy | clean | local pass |
| release-check | fmt, clippy, tests, HTTP e2e, claims, TUI PTY; attribution red on local history | local pass, rebuilt history required |
| 2h crash-injection soak | 28,552 cycles / 7,200.20s / exit 0 | local pass |
| Structural replay | 100% on completed journals | local pass |
| Audit completeness | 100% under `AgentAnswer::validate` | local pass |
| 1h real-task gate | 30 runs / 240 tasks: M1 97.08%, suite 76.66%, M5 100%, 44m16s, ¥11.63 | local pass; one lost case record |
| 8h local-model soak | 56 runs / 448 tasks, 0 crashes, replay 234/234, 8h26m51s | runtime pass; local model quality 21.2%, not M1 |
| 100-question AG-23 | 96/100, 0 hard gate | local pass |
| M7 TTFT | n=20 p50 1.50 s / p95 2.77 s | one provider; tools disabled |
| M19–M23 | M19 85.00%, M20 5.13% (2/39), M21 2.50%, M22 0.00%, M23 null | **AI/bulk labels, not human blind review** |
| Independent code-level review | 0 P0; five P1 classes fixed; four P1 residual | not a third-party audit |
| Third-party audit / macOS P-Gate | not started | pending |

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
- Human blind review, third-party A-Gate/W-Gate/security certification and
  macOS P-Gate remain open; M19–M23 currently come from AI/bulk labels and
  must not be described as a human blind-review pass.
- `docs/audit-v0.97.md` lists four open P1 residual items (browser_read SSRF,
  run-journal hash chain, cross-builder reproducibility, historical
  attribution). Do not claim a passed security audit.
- No 8h DeepSeek model-quality claim is made; the M1 quality claim is limited
  to the ≤1h task scope.

## Upgrade and rollback

1. Back up the runtime home (`~/.1a`) and journal directories.
2. The 0.97 binary reads older journals; the reverse is not guaranteed.
3. See `docs/rollback-v0.97.md` for rollback points and steps.
4. The `~/.1a/KILL` kill switch blocks write tools; it does not kill a process,
   restart a daemon or clear locks.

## Release assets (local candidate)

- `1agent-0.97.0-linux-x86_64.tar.gz` — Linux x86_64 package;
- `SHA256SUMS` — archive and provenance checksums;
- `linux-x86_64.json` — Linux build provenance.

The local package is built and verified: binary SHA-256
`36c6e44faa15e4011261f20b735738cd7fb05a9b9818ddd0340e0fad482ee3fb`,
`package.sh --linux-only` built twice with `reproducible=true`, and inner/outer
`SHA256SUMS`, provenance, `--version`, init/doctor and a temp-prefix install all
passed. This is a local release candidate, not a public announcement; a public
release still needs rebuilt clean history and asset confirmation.
