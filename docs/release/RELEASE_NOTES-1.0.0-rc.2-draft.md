# 1Agent 1.0.0-rc.2 — code-level release candidate (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.1 plus the permission UX fix

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What changed in rc.2

- **Permission UX**: the model cannot change its own permissions. In the TUI,
  network/write access is enabled for the next run with
  `/permissions network on` or `/permissions write on`; no TUI restart is
  needed. Headless runs use `--allow-network`/`--read-only`.
- Clarify options must be answerable by the human; the model may not offer an
  option that promises to enable a permission it cannot grant.
- Network-denied tool errors and `describe_self` now state the exact TUI
  command instead of only `--allow-network`.
- Includes rc.1: QG1 goal-closure evidence, Track Q 99/100, the high-risk
  repaired-answer safety gate, 0.98/0.99 kernels and the 1.0 code-level gates.

## 2. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1638 total, 0 failed, 13 ignored |
| `agent-loop-v1` rc.2 | 99/100, 0 hard-gate, audit/replay 100/100, rubric 20/20 |
| QG1 goal closure | pass; missing/mismatched/self/rejected/hacked fail closed |
| HTTP surface e2e | 1/1 |
| TUI interaction | pass; two-Ctrl+Q quit and permission command coverage |
| 5-cycle soak | pass |
| Python SDK | 6/6 |
| Windows GNU check/link | pass; macOS blocked by host C toolchain |
| `release-check.sh` | software gates pass; `ci.sh` blocked at attribution history |

## 3. Known limits / blocked

- `intro-20`: used 6 tool calls where 4 are allowed (three `describe_self`
  calls); efficiency only, no hard gate. `chat-19` passed in this run — both
  runs are one provider/model sample, so the residual moved rather than
  disappeared.
- CQ3/CQ6/CQ7/CQ9/CQ12/CQ16 remain Partial in the 0.99 ontology matrix.
- No 8h DeepSeek model-quality claim; the prior 8h local-model soak is
  runtime-only evidence.
- No third-party audit/certification, real Windows/macOS GA, signing,
  notarization, marketplace, mobile or R1–R9 metrics.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 4. Local assets

To be filled after the rc.2 package is built. No GitHub release and no push are
performed by the code-level candidate workflow.
