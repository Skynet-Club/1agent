# 1Agent 1.0.0-rc.10 — step-budget finalization (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.9 plus step-budget finalization

## 1. What changed

- When the **step budget** is exhausted after an `acceptance_gate` receipt was
  accepted, the runtime now makes one final tool-free answer turn. This mirrors
  the existing tool-call-budget behavior and prevents a verified artifact from
  being discarded as `needs_evidence`.
- If no accepted receipt exists, the runtime keeps the honest
  `needs_evidence` budget answer. A completed claim is never fabricated from a
  partial run.
- The finalize prompt now says “run budget” instead of “tool-call budget,” so
  both step and tool-call exhaustion share the same contract.

## 2. Why the previous smoke stopped at six steps

`1agent run` defaults to `--max-steps 6`, `--max-tool-calls 8`,
`--max-provider-attempts 8`. These are headless cost/safety guards, not task
quality limits. A complex artifact pipeline (resolve capability → search →
write → preview → browser audit → acceptance) needs more than six steps. The
old step-budget path ended with a canned `needs_evidence` answer before the
final answer turn. The new path only finalizes when there is an accepted
receipt; otherwise it still fails closed.

## 3. Verification

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1657 total, 0 failed, 13 ignored |
| step-budget test | still `needs_evidence` without an accepted receipt |
| tool-budget tests | unchanged |
| capability compiler smoke (rc.10) | final answer `conditional`; stop `completed`; SVG+HTML, preview HTTP 200, audit 0 overflow/0 placeholders, acceptance accepted |

## 4. Limits

- The six-step default remains a conservative headless default; artifact tasks
  should pass `--max-steps`/`--max-tool-calls` or use the unlimited interactive
  TUI. A complexity-aware default is future work.
- P1 runtime preflight/artifact verifier and P2 strategy memory remain open.
- Human calibration, 8h DeepSeek, external audit, real Windows/macOS, R1–R9
  remain blocked.

## 5. Local assets

Built from tag `v1.0.0-rc.10` → `a7bbfa6` with `./scripts/package.sh --linux-only`;
tree clean, `reproducible=true`.

| Asset | Value |
|---|---|
| `1agent-1.0.0-rc.10-linux-x86_64.tar.gz` | SHA-256 `3eb3692ce7f4f48e7d1febf9f76870c53930886076b5128607ab2f081b926bc4` |
| Packaged binary | SHA-256 `df95495a62b2e3331c7d28c3f79ca7b0a502c444f819068ad81465d38eb3ec2c` |
| Provenance | `linux-x86_64.json`, commit `a7bbfa6`, `git_dirty: false` |
| Smoke | SHA256SUMS OK; `--version` 1.0.0-rc.10; install to `~/.local` OK |

Evidence: `docs/evidence/release-1.0.0-rc.10-2026-11-18.md`. No GitHub release
and no push were performed.
