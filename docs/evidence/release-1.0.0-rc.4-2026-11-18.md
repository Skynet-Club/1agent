# 1.0.0-rc.4 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.4` → `23d12cd`
  (`fix: stop TUI right-click from pasting stale clipboard`)
- Version in `Cargo.toml`: `1.0.0-rc.4`
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
| `dist/1agent-1.0.0-rc.4-linux-x86_64.tar.gz` | `36cd694d7c965a92d8a73ebe80b377d0a2901aba1b9975cd19ff5abb966582ff` |
| Linux binary inside archive | `971451e6622bdd9ee5eaf94f4bd7e52c9141719883b0de1b5269c54c26583bbb` |
| Binary size | 25,860,968 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` as `729b7d24…` |

Provenance: `schema_version 1`, package `first-principles-agent` `1.0.0-rc.4`,
`x86_64-unknown-linux-gnu`, commit
`23d12cd9bf461008c1443769ef0973aa8f78fce9`, `git_dirty: false`.

## 4. Smoke

- Extracted to `/tmp/rc4-smoke2/1agent-1.0.0-rc.4`.
- `sha256sum -c SHA256SUMS` inside the archive → all files **OK**.
- `1agent --version` → `1agent 1.0.0-rc.4`.
- `1agent doctor` → `overall: Ok`, `full-local` with network true.
- `scripts/install.sh --prefix ~/.local` → installed binary is
  `1.0.0-rc.4`.
- PTY test with a fake `wl-paste` marker: right-click in the composer leaves
  the screen unchanged; `Ctrl+V` still inserts the marker. Log:
  `docs/evidence/1.0-local-qa-2026-11-17/right-click-rc.4.log`.

Raw log: `docs/evidence/1.0-local-qa-2026-11-17/package-rc.4.log`.

## 5. Explicit non-claims

- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push, no marketplace publication.
- `release-check.sh` still blocks at the historical attribution check; the
  local package was built from the tagged tree, not from a clean public
  history.
- External audit and R1–R9 metrics remain blocked.
