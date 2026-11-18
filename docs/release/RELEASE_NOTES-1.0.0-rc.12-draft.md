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

To be filled after packaging from the clean rc.12 tag.
