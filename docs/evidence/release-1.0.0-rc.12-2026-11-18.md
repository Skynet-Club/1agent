# 1.0.0-rc.12 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package.

- Tag: `v1.0.0-rc.12` → `41c24ec`
- Archive: `649e66aa68b993aa29b0e96bdeae49e6f4eccde7aa604da6b5dd6f14d4cbb293`
- Binary: `26fc0988820276e7e55ff9bacfe72f4084227a54d881e75a41de8a054c9819b1`
  (26,713,904 bytes)
- Provenance: commit `41c24ec474bec8a01ccbeb51c3a0caa832935fed`,
  `git_dirty: false`, `reproducible=true`
- SHA256SUMS OK; `--version` 1.0.0-rc.12; installed to `~/.local`

## Smoke

- `run --provider mock --task "生成一个 3D 模型"` → `needs_authorization`,
  stop `needs_authorization`; no `model_responded` event.
- `capability resolve --task "画一张架构图"` → selected `manual-svg`;
  alternatives `archify`, `mermaid`.
- Live 12-step provider run: final `completed`; SVG verified; accepted receipt;
  `~/.1a/capability-paths.jsonl` records `manual-svg` with
  `artifact_verified:true`.
- `doctor`: overall Ok; provider deepseek/deepseek-flash credential present;
  permissions full-local (write/network true).
- Focused tests: `accepted_receipt_survives_unrepairable_plain_text_final`,
  `preflight_path_is_recorded_for_a_completed_artifact_run`.

Verification: `verify-1.0.0-rc.12.log`; 1668 total / 0 failed / 13 ignored;
claims 264 (240 verified / 16 partial / 8 no evidence).

No GitHub release, no push; external audit, human calibration, 8h DeepSeek,
real Windows/macOS GA, marketplace and R1–R9 remain blocked.
