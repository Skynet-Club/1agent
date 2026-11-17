# 1.0.0-rc.1 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.1` → `88ad7e6`
  (`release: cut local 1.0.0-rc.1 code-level candidate`)
- Version in `Cargo.toml`: `1.0.0-rc.1`
- Tree: clean at package time (pre-existing 0.98.2 evidence directories are
  locally excluded; no source changes were made after the tag).

## 2. Command

```bash
./scripts/package.sh --linux-only
```

Result: **exit 0**, `reproducible=true` (two builds produced the same binary
SHA), target `x86_64-unknown-linux-gnu`.

## 3. Artifacts

| Artifact | SHA-256 / value |
|---|---|
| `dist/1agent-1.0.0-rc.1-linux-x86_64.tar.gz` | `c39762c8f0348316cfe08b4262fcd1f16705fcde3f5b58481e6838d4980075d6` |
| Linux binary inside archive | `da2d006b781337e4fca0fb32c41ada616f2c3279ed89a709134d397b270c0605` |
| Binary size | 25,855,848 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` as `27a9acf4…` |

The archive contains the binary, install/upgrade/uninstall scripts, README,
CHANGELOG, licenses, security/privacy docs and provenance.

## 4. Smoke

- Extracted to an isolated prefix under `$HOME/.cache/1agent-tmp/rc-install`.
- `1agent --version` → `1agent 1.0.0-rc.1`.
- `1agent doctor --json` → `status: ok` on the existing runtime home.
- Archive SHA matched `dist/SHA256SUMS`.

Raw log: `docs/evidence/1.0-local-qa-2026-11-17/package.log`.

## 5. Explicit non-claims

- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push, no marketplace publication.
- `release-check.sh` still blocks at the historical attribution check; the
  local package was built from the tagged tree, not from a clean public
  history.
- External audit and R1–R9 metrics remain blocked.
