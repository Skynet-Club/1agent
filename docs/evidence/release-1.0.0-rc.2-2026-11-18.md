# 1.0.0-rc.2 local package evidence (2026-11-18)

Scope: local Linux x86_64 release-candidate package from a clean tagged tree.
This is not a public release and not GA.

## 1. Source

- Branch: `0.99-dev`
- Tag: `v1.0.0-rc.2` → `d1e82f2`
  (`fix: make TUI permission remedies consistent and cut 1.0.0-rc.2`)
- Version in `Cargo.toml`: `1.0.0-rc.2`
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
| `dist/1agent-1.0.0-rc.2-linux-x86_64.tar.gz` | `52d45aa559eda29adc47c47765205d7b857a0aa8f5f7200081c32b0e3169fcff` |
| Linux binary inside archive | `6fd90537fd227a0895175522b25cd9bda9fb258bde95728090f51e345fceb9a8` |
| Binary size | 25,856,456 bytes |
| `dist/linux-x86_64.json` | listed in `dist/SHA256SUMS` as `00ea362c…` |

Provenance: `schema_version 1`, package `first-principles-agent` `1.0.0-rc.2`,
`x86_64-unknown-linux-gnu`, `rustc 1.99.1`, `cargo 1.99.1`, commit
`d1e82f25e843a453f8ef20223c16ab251e02261b`, `git_dirty: false`.

The archive contains the binary, install/upgrade/uninstall scripts, README,
CHANGELOG, licenses, security/privacy docs, docs/evals/configs and provenance.

## 4. Smoke

- Extracted to `/tmp/rc2-smoke/1agent-1.0.0-rc.2`.
- `sha256sum -c SHA256SUMS` inside the archive → all files **OK**.
- `1agent --version` → `1agent 1.0.0-rc.2`.
- `1agent doctor` → `overall: Ok`.
- `scripts/install.sh --prefix /tmp/rc2-prefix` → installed; installed binary
  `--version` is `1.0.0-rc.2` and its SHA-256 matches the packaged binary.

Raw log: `docs/evidence/1.0-local-qa-2026-11-17/package-rc.2.log`.

## 5. Explicit non-claims

- No real Windows/macOS package, signing, notarization or install smoke.
- No GitHub Release, no push, no marketplace publication.
- `release-check.sh` still blocks at the historical attribution check; the
  local package was built from the tagged tree, not from a clean public
  history.
- External audit and R1–R9 metrics remain blocked.
