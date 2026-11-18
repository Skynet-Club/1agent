# 1.0.0-rc.8 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.8` → `7ca907f`
  (`fix: treat local absolute paths as tasks, not slash commands`)
- Version in `Cargo.toml`: `1.0.0-rc.8`
- Tree: clean at package time; provenance records `git_dirty: false` and the
  same commit.

## 2. Command

```bash
./scripts/package.sh --linux-only
```

Result: **exit 0**, `reproducible=true`, target `x86_64-unknown-linux-gnu`.

## 3. Artifacts

| Artifact | SHA-256 / value |
|---|---|
| `dist/1agent-1.0.0-rc.8-linux-x86_64.tar.gz` | `328230ea14dd69ef0ed103833f629a5bd3c2815b61fd00317c249a264c48ac51` |
| Linux binary inside archive | `d235255d60a981716e3f32f74e8b6b6dea959dd9af5e487b58a55eadbf4a8fd0` |
| Binary size | 26,009,264 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` |

Provenance: `schema_version 1`, package `first-principles-agent` `1.0.0-rc.8`,
commit `7ca907f6680cc7575819e508e8d884add5386538`, `git_dirty: false`.

## 4. Smoke

- `sha256sum -c SHA256SUMS` inside the archive → all files **OK**.
- `1agent --version` → `1.0.0-rc.8`.
- `scripts/install.sh --prefix ~/.local` → installed `1.0.0-rc.8`.
- Installed TUI smoke: submitting
  `/home/skynet/文档/Skynet-Docs/Skynet-Club/skynet-club/01-理想社区需求规格.md`
  starts a task and does **not** print `Unknown command`.

## 5. Explicit non-claims

- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push, no marketplace publication.
- External audit, 8h DeepSeek quality, human calibration and R1–R9 remain
  blocked.
