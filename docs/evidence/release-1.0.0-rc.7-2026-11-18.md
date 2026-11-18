# 1.0.0-rc.7 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.7` → `04c7c86`
  (`feat: add repeat/pass^k evaluation and process metrics`)
- Version in `Cargo.toml`: `1.0.0-rc.7`
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
| `dist/1agent-1.0.0-rc.7-linux-x86_64.tar.gz` | `f46a7152f7e3a46f7a2e67f4c8e4f773d374e0ee729ea758e78db163afeb1be8` |
| Linux binary inside archive | `bb07d72e32c63a573761bff28f9a813eb45f771aef7fe4deb661ffeb83c79f76` |
| Binary size | 26,007,856 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` |

Provenance: `schema_version 1`, package `first-principles-agent` `1.0.0-rc.7`,
`x86_64-unknown-linux-gnu`, commit
`04c7c863aa07d2579dfad42c2d7a3980a92f11b3`, `git_dirty: false`.

## 4. Smoke

- Extracted to `/tmp/rc7-smoke/1agent-1.0.0-rc.7`.
- `sha256sum -c SHA256SUMS` inside the archive → all files **OK**.
- `1agent --version` → `1agent 1.0.0-rc.7`.
- `1agent doctor` → `overall: Ok`, `full-local` with network true.
- `scripts/install.sh --prefix ~/.local` → installed binary is `1.0.0-rc.7`.
- `eval --repeats 2 --suite evals/core.json` → 16/16 attempts, `pass^2`
  100%; real-model 4-case smoke → 7/8, `pass^2` 75%, `chat-19` flaky.

Raw logs: `verify-1.0.0-rc.7.log`, `package-rc.7.log`;
repeat evidence: `docs/evidence/eval-repeatability-2026-11-18/`.

## 5. Explicit non-claims

- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push, no marketplace publication.
- `release-check.sh` still blocks at the historical attribution check; the
  local package was built from the tagged tree, not from a clean public
  history.
- External audit, 8h DeepSeek quality, human calibration and R1–R9 remain
  blocked.
