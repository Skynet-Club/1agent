# 1.0.0-rc.5 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.5` → `52fcd67`
  (`feat: add /setting and eight TUI themes, cut 1.0.0-rc.5`)
- Version in `Cargo.toml`: `1.0.0-rc.5`
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
| `dist/1agent-1.0.0-rc.5-linux-x86_64.tar.gz` | `c8b74fc769d75f7f589c8e57d1dc4007af4b8e4a2c447692d2d4a36fb8278ee9` |
| Linux binary inside archive | `55042418fba07c654561cfd80df63aeb11e6b063cb15759408478584ddf7e409` |
| Binary size | 25,915,872 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` as `855e77a6…` |

Provenance: `schema_version 1`, package `first-principles-agent` `1.0.0-rc.5`,
`x86_64-unknown-linux-gnu`, commit
`52fcd67b5deb3cbbfc42dce8ffb5c6fd962e3ee6`, `git_dirty: false`.

## 4. Smoke

- Extracted to `/tmp/rc5-smoke/1agent-1.0.0-rc.5`.
- `sha256sum -c SHA256SUMS` inside the archive → all files **OK**.
- `1agent --version` → `1agent 1.0.0-rc.5`.
- `1agent doctor` → `overall: Ok`, `full-local` with network true.
- `scripts/install.sh --prefix ~/.local` → installed binary is
  `1.0.0-rc.5`.
- Installed TUI `/setting show` reports the military theme and current values;
  `/setting theme matrix` switches to matrix green (no config in the isolated
  home, so it reports “仅本次生效”).
- PTY settings/theme e2e and reset tests pass.

Raw logs: `verify-1.0.0-rc.5.log`, `tui-interaction-rc.5.log`,
`package-rc.5.log`.

## 5. Explicit non-claims

- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push, no marketplace publication.
- `release-check.sh` still blocks at the historical attribution check; the
  local package was built from the tagged tree, not from a clean public
  history.
- External audit and R1–R9 metrics remain blocked.
