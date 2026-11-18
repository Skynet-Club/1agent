# 1.0.0-rc.11 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package.

- Tag: `v1.0.0-rc.11` → `f352ba8`
- Archive: `3f8dc770ece6dff085b4b58bf8e2ec4a08201b90b878fcd6aedf1b80bac3c8bd`
- Binary: `d469f9cfae365beec954435739e253f0e19d83e1c80749af4c7c9d8d6a7bda56`
  (26,385,552 bytes)
- Provenance: commit `f352ba87b8abecfcef254207da07c3cded63714e`,
  `git_dirty: false`, `reproducible=true`
- SHA256SUMS OK; `--version` 1.0.0-rc.11; installed to `~/.local`

## Smoke

- `run --provider mock --task "生成一个 3D 模型"` → `needs_authorization`,
  stop `needs_authorization`; preflight journal has no model request.
- `capability resolve --task "画一张架构图"` → selected `manual-svg`;
  alternatives `archify`, `mermaid`.
- `capability propose-install --name archify` → exact Hermes command.
- `capability approve-install --yes` → approver `skynet`, `executed:false`,
  approval ledger written; no install executed.
- `verify()` covers runtime preflight, artifact verifier, completion gate,
  capability memory and CLI proposal/approval.

Verification: `verify-1.0.0-rc.11.log`; claims 263 (239/16/8).

No GitHub release, no push; external audit, human calibration, 8h DeepSeek,
real Windows/macOS GA, marketplace and R1–R9 remain blocked.
