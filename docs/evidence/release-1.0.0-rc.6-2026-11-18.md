# 1.0.0-rc.6 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.6` → `68d7a89`
  (`fix: make /setting a two-level in-panel settings surface`)
- Version in `Cargo.toml`: `1.0.0-rc.6`
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
| `dist/1agent-1.0.0-rc.6-linux-x86_64.tar.gz` | `6cf7cc8e8fce2e21c6c4f1cc39f6eff01d671aafc30e01c942ff32ad95d5fec2` |
| Linux binary inside archive | `641575e2e1212132c86343a3fd4370b7b0f0dcae92c524f0a9541384561aaa0c` |
| Binary size | 25,923,616 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` |

Provenance: `schema_version 1`, package `first-principles-agent` `1.0.0-rc.6`,
`x86_64-unknown-linux-gnu`, commit
`68d7a89e2f0a40e12e602403440b190b4988bf91`, `git_dirty: false`.

## 4. Smoke

- Extracted to `/tmp/rc6-smoke/1agent-1.0.0-rc.6`.
- `sha256sum -c SHA256SUMS` inside the archive → all files **OK**.
- `1agent --version` → `1agent 1.0.0-rc.6`.
- `1agent doctor` → `overall: Ok`, `full-local` with network true.
- `scripts/install.sh --prefix ~/.local` → installed binary is
  `1.0.0-rc.6`.
- PTY settings panel: `/setting` → budget → inline step edit → panel shows
  `已更新设置：steps`; no command is inserted into the composer.
- TUI interaction check passes with the fresh release binary.

Raw logs: `verify-1.0.0-rc.6.log`, `tui-interaction-rc.6.log`,
`settings-panel-rc.6.log`, `package-rc.6.log`.

## 5. Explicit non-claims

- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push, no marketplace publication.
- `release-check.sh` still blocks at the historical attribution check; the
  local package was built from the tagged tree, not from a clean public
  history.
- External audit and R1–R9 metrics remain blocked.
