# 1Agent 1.0.0-rc.7 — evaluation depth (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.6 plus evaluation process metrics and repeatability

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What changed in rc.7

- `eval --repeats N` runs a suite N times and reports:
  - overall attempt pass rate;
  - `pass^k` (all k attempts pass) and at-least-once pass rate;
  - per-case attempts, passes, hard-gate failures and failure classes;
  - `--min-pass-power-k-bps` as an optional reliability gate.
- `EvalCaseResult` now records process metrics derived from the run journal:
  `tool_calls`, `failed_tool_calls`, `redundant_tool_calls`,
  `time_to_first_tool_ms`.
- `FailureClass` gives deterministic, rule-based attribution:
  planning, tool selection, tool arguments, evidence, generation, budget,
  safety, provider, infra, unknown. `EvalReport` carries
  `failure_class_counts`.
- `docs/evaluation.md` documents the three-layer model, metrics, repeat
  semantics and remaining gaps.

## 2. Repeatability smoke (real model)

A four-case subset (`intro-03`, `intro-20`, `chat-19`, `risk-12`) was run twice
with the configured DeepSeek provider:

| Metric | Value |
|---|---:|
| Attempts | 8 |
| Passed attempts | 7 |
| Overall pass rate | 87.50% |
| `pass^2` | 75.00% |
| At least once | 100% |
| Failure classes | `budget` × 1 |

`chat-19` used 2 provider calls in one attempt (limit 1) and 1 in the other.
This is the instability a single 99/100 run hides. Evidence:
`docs/evidence/eval-repeatability-2026-11-18/`.

## 3. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1651 total, 0 failed, 13 ignored |
| claims table | well-formed; 260 checked (236 verified / 16 partial / 8 no evidence) |
| Eval unit tests | failure classes, ratios, repeated suite pass^k |
| CLI repeat smoke | `eval --repeats 2` on `core-v3` and the real-model subset |

## 4. Known limits / blocked

- Human calibration set is **not** done. The 0.97 labels are AI/bulk-written,
  not human blind review; the harness and docs describe the required process.
- 8h DeepSeek model-quality gate is **not** run (cost/time not approved).
- R1–R9 real-user metrics remain blocked.
- Failure classes are rule-derived, not semantic judgments.
- `intro-20` efficiency and CQ3/CQ6/CQ7/CQ9/CQ12/CQ16 remain open.
- No third-party audit/certification, real Windows/macOS GA, signing,
  notarization, marketplace or mobile.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 5. Local assets

To be filled after the rc.7 package is built. No GitHub release and no push are
performed by the code-level candidate workflow.
