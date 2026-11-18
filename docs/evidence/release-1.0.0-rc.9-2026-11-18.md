# 1.0.0-rc.9 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.9` → `fe0c739`
  (`feat: add open-world capability compiler and resolve tool`)
- Version in `Cargo.toml`: `1.0.0-rc.9`
- Tree: clean at package time; provenance records `git_dirty: false`.

## 2. Artifacts

| Artifact | SHA-256 / value |
|---|---|
| `dist/1agent-1.0.0-rc.9-linux-x86_64.tar.gz` | `914ec66875cdd8118697d65b85476b04d8b4c45cfdab552bd9279cf7bb97a26a` |
| Linux binary | `ec3bef264921e8d65504dd3d464ea337a48921c0c2368d5411a29366c31a4e92` |
| Binary size | 26,163,296 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` |

Provenance: `first-principles-agent` `1.0.0-rc.9`, `x86_64-unknown-linux-gnu`,
commit `fe0c73911fea9b8b1b428419778fb4b048c09367`, `git_dirty: false`.

## 3. Smoke

- `sha256sum -c SHA256SUMS` → OK.
- `--version` → 1.0.0-rc.9; installed to `~/.local`.
- Capability resolver tests pass.
- Real-model behavior smoke called `resolve_capability`, saw the missing
  renderer and `archify` alternative, wrote and previewed an SVG, and
  `acceptance_gate` accepted 5 criteria; final status was `needs_evidence`
  because the step budget ran out. Evidence:
  `docs/evidence/capability-compiler-2026-11-18/`.

## 4. Explicit non-claims

- P0 resolver/tool/prompt only; runtime preflight enforcement and artifact
  verifier are P1.
- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push.
- External audit, human calibration, 8h DeepSeek quality and R1–R9 remain
  blocked.
