# 1Agent 1.0.0-rc.1 — code-level release candidate (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 0.98 Real World + 0.99 organization kernels + 1.0 code-level hardening

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What this candidate contains

- **0.98 Real World**: controlled browser/computer-use policy, MCP HTTP/SSE
  and OAuth/token-exchange paths, skill capture/lifecycle/canary, daemon and
  Inbox/attention projections, partner voice forms, onboarding and context
  import, attachments, A2A/Agent Card and trust disclosures.
- **0.99 Goal / Action / Delegation kernels**: Goal→Run binding, close with
  evidence, commitment lifecycle, ActionRecord + policy/approval/reconciliation,
  child-run lineage and failure/timeout/cancel matrix, ontology projections,
  `goal`/`action` CLI.
- **1.0 code-level hardening**: QG1 goal closure now requires a stored
  `VerificationStore` record (run match, `Verified`, executor≠verifier);
  high-risk repaired answers cannot bypass the evidence re-plan; budget stop
  reasons follow the final answer status; Track Q final 99/100.
- **SDK/API**: v0 HTTP surface with OpenAPI; Python stdlib client covers
  health/openapi/agent-card/trust-center/surface.
- **Platform code gate**: Linux full; Windows GNU all-targets check + release
  link; macOS host-toolchain block recorded.

## 2. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass: 1196 lib / 1637 total / 0 failed, core-v3 8/8 |
| `agent-loop-v1` final | **99/100**, 0 hard-gate, audit/replay 100/100, rubric 20/20 |
| QG1 goal closure | pass; missing/mismatched/self/rejected/hacked records fail closed |
| HTTP surface e2e | 1/1 |
| TUI interaction | pass; Backspace safety, one-press arm/Esc cancel/two-press quit, 400 mixed edits |
| 5-cycle soak | pass |
| Python SDK | 6/6 |
| Windows GNU check/link | pass; `.exe` SHA-256 `535eb260…` |
| macOS arm64 | blocked by host C toolchain (`ring/cc`) |
| `release-check.sh` | fmt/clippy/tests/http pass; `ci.sh` blocked at attribution history |

## 3. Known limits / blocked

- `chat-19`: a simple chat turn used 2 provider calls where 1 is allowed.
- CQ3/CQ6/CQ7/CQ9/CQ12/CQ16 remain Partial in the 0.99 ontology matrix.
- No 8h DeepSeek model-quality claim; the prior 8h local-model soak is
  runtime-only evidence.
- No third-party audit/certification, real Windows/macOS GA, signing,
  notarization, marketplace, mobile or R1–R9 metrics.
- `browser_read` Chrome SSRF and journal hash-chain tamper evidence remain
  residual security items from the 0.97 review.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 4. Local assets

To be filled after the local package is built. No GitHub release and no push
are performed by the code-level candidate workflow.
