# 1.0.0-rc.3 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.3` → `ccb9e11`
  (`feat: enable full-power local defaults and cut 1.0.0-rc.3`)
- Version in `Cargo.toml`: `1.0.0-rc.3`
- Tree: clean at package time; provenance records `git_dirty: false` and the
  same commit.

## 2. Command

```bash
./scripts/package.sh --linux-only
```

Result: **exit 0**, `reproducible=true` (two builds produced the same binary
SHA), target `x86_64-unknown-linux-gnu`.

## 3. Artifacts

| Artifact | SHA-256 / value |
|---|---|
| `dist/1agent-1.0.0-rc.3-linux-x86_64.tar.gz` | `faee878664755741ae9ac77df9d2966ec6c4dfa1f6a5c0204853d8db224ba987` |
| Linux binary inside archive | `cfec45b9ee72032b88b66722faf558a5bc12209956f428b123a30466d740e96b` |
| Binary size | 25,861,288 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` as `d863bd34…` |

Provenance: `schema_version 1`, package `first-principles-agent` `1.0.0-rc.3`,
`x86_64-unknown-linux-gnu`, commit
`ccb9e11637be78be0f892753b65bded9d44fbd6b`, `git_dirty: false`.

## 4. Smoke

- Extracted to `/tmp/rc3-smoke/1agent-1.0.0-rc.3`.
- `sha256sum -c SHA256SUMS` inside the archive → all files **OK**.
- `1agent --version` → `1agent 1.0.0-rc.3`.
- `1agent doctor` → `overall: Ok`; on the existing `full-local` config it
  reports `network: true` even though the file still has a stale
  `allow_network: false` field.
- `scripts/install.sh --prefix /tmp/rc3-prefix` and `--prefix ~/.local` →
  installed binary `--version` is `1.0.0-rc.3` and SHA-256 matches the package.
- Installed TUI `/permissions show` → `网络访问：允许`, `写入与执行：允许`.

Raw log: `docs/evidence/1.0-local-qa-2026-11-17/package-rc.3.log`.

## 5. Explicit non-claims

- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push, no marketplace publication.
- `release-check.sh` still blocks at the historical attribution check; the
  local package was built from the tagged tree, not from a clean public
  history.
- External audit and R1–R9 metrics remain blocked.
