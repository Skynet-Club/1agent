# 1Agent 1.0.0-rc.12 — final-answer preservation (draft)

Date: 2026-11-18
Status: **local code-level release candidate; not published, not GA**
Scope: 1.0.0-rc.11 plus preservation of verified delivery when the final answer
cannot be repaired

## 1. What changed

- When both answer-repair attempts fail after an accepted `acceptance_gate`
  receipt or a `verify_artifact` result, the plain-text final answer is
  normalized into a structured answer instead of being discarded as
  `needs_evidence`.
- The fallback only applies to non-JSON prose; malformed JSON and fenced
  output still fail closed.
- `CompletionGate` still runs: missing acceptance receipts or design evidence
  downgrade the answer to `conditional`. The fallback does not grant a pass on
  its own.
- Verified artifact paths and SHA-256 values are attached as evidence when the
  model's prose does not already name them.
- The runtime preflight resolution is journaled as
  `capability_preflight_resolved`, and capability-path memory reads that
  authoritative path before the model's optional resolver call. This makes P2
  memory observable even when the model's later resolver call describes a
  narrower contract.

## 2. Verification

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1668 total, 0 failed, 13 ignored |
| claims | 264 checked (240 verified / 16 partial / 8 no evidence) |
| focused tests | `accepted_receipt_survives_unrepairable_plain_text_final`, `preflight_path_is_recorded_for_a_completed_artifact_run` |
| live smoke | `completed`; verified SVG + accepted receipt; `manual-svg` path recorded with `artifact_verified:true` |

## 3. Limits

- The normalized answer has no structured claims; its conclusion remains the
  model's narration. Only the runtime-verified artifact/receipt is structural
  evidence.
- JSON-shaped or fenced responses are not normalized.
- Human calibration, 8h DeepSeek, external audit, real Windows/macOS, R1–R9
  remain blocked.

## 4. Local assets

Built from tag `v1.0.0-rc.12` → `41c24ec` with `./scripts/package.sh
--linux-only`; tree clean, `reproducible=true`.

| Asset | Value |
|---|---|
| `1agent-1.0.0-rc.12-linux-x86_64.tar.gz` | SHA-256 `649e66aa68b993aa29b0e96bdeae49e6f4eccde7aa604da6b5dd6f14d4cbb293` |
| Packaged binary | SHA-256 `26fc0988820276e7e55ff9bacfe72f4084227a54d881e75a41de8a054c9819b1` (26,713,904 bytes) |
| Provenance | `linux-x86_64.json`, commit `41c24ec`, `git_dirty: false` |
| Smoke | SHA256SUMS OK; `--version` 1.0.0-rc.12; install to `~/.local`; `doctor` Ok |

Evidence: `docs/evidence/release-1.0.0-rc.12-2026-11-18.md`. No GitHub release
and no push were performed.
