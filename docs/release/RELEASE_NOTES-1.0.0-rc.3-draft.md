# 1Agent 1.0.0-rc.3 — full-power defaults (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.2 plus full-power default permissions

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What changed in rc.3

- Local TUI and `run` now start at **full power by default**: file writes,
  terminal execution and network tools are enabled without flags.
- `--no-network` disables network for one run; `--read-only` disables writes.
  `/permissions network off` in the TUI still applies to the next run only.
- The TUI now honors the configured permission preset. Previously a
  `full-local` config with `allow_network: false` was ignored by the TUI, so
  the session stayed offline; the preset now resolves to network on.
- New installs default to the `full-local` preset. Existing explicit
  `safe`/`read-only` configs keep their configured gate.
- `--allow-network` remains accepted as a hidden compatibility flag; new
  scripts should omit it. `eval`/`benchmark-run` keep their explicit
  `--allow-network` switch so benchmark runs stay reproducible.
- The rc.2 permission-UX fix is included: the model cannot grant itself
  permissions; denied tools state the human command.

## 2. Security tradeoff

Default network widens the prompt-injection egress surface. High-risk and
irreversible actions still require Authority/Policy and confirmation, and the
run-time permission snapshot still cannot be changed by the model. Two known
residuals become more important with network on by default:

- O1: `browser_read` can follow DNS/redirects/subresources to
  metadata/loopback targets; a validating proxy or pinned resolver is needed.
- O2: run journals have no per-event hash chain or external anchor.

Use `--no-network` for untrusted pages or hermetic runs; do not treat default
network as an SSRF fix.

## 3. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1640 total, 0 failed, 13 ignored |
| claims table | well-formed; 258 checked (234 verified / 16 partial / 8 no evidence) |
| TUI interaction | pass (real provider; two-Ctrl+Q quit) |
| Network default / opt-out | `tui_network_permission_defaults_on_and_no_network_disables_it` pass |
| Config full-local | TUI resolves `full-local` to network on even with a stale `allow_network: false` field |
| Track Q | rc.2 full run 99/100; eval keeps explicit network flag, so this default change does not alter benchmark inputs |

## 4. Known limits / blocked

- `intro-20` efficiency residual from the rc.2 Track Q run (6 tool calls vs 4).
- CQ3/CQ6/CQ7/CQ9/CQ12/CQ16 remain Partial in the 0.99 ontology matrix.
- Network default on increases exposure to O1/O2 until those are fixed.
- No third-party audit/certification, real Windows/macOS GA, signing,
  notarization, marketplace, mobile or R1–R9 metrics.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 5. Local assets

Built from tag `v1.0.0-rc.3` → `ccb9e11` with `./scripts/package.sh
--linux-only`; tree clean, `reproducible=true`.

| Asset | Value |
|---|---|
| `1agent-1.0.0-rc.3-linux-x86_64.tar.gz` | SHA-256 `faee878664755741ae9ac77df9d2966ec6c4dfa1f6a5c0204853d8db224ba987` |
| Packaged binary | SHA-256 `cfec45b9ee72032b88b66722faf558a5bc12209956f428b123a30466d740e96b` |
| Provenance | `linux-x86_64.json`, commit `ccb9e11`, `git_dirty: false` |
| Smoke | `sha256sum -c SHA256SUMS` OK; `--version` 1.0.0-rc.3; `doctor` Ok; install smoke OK; installed TUI shows `网络访问：允许` |

Evidence: `docs/evidence/release-1.0.0-rc.3-2026-11-18.md`. No GitHub release
and no push were performed.
