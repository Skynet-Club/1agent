# 1Agent 1.0.0-rc.4 — right-click paste fix (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.3 plus the TUI right-click fix

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What changed in rc.4

- Right-click in the TUI composer no longer pastes. The old binding inserted
  whatever was in the system clipboard, so stale content survived restarts and
  was reinserted on every right-click. Paste is now explicit: `Ctrl+V` or
  `Shift+Insert`.
- The fix is covered by a PTY test: right-click leaves the composer unchanged;
  `Ctrl+V` still pastes the clipboard.
- Includes rc.3 full-power defaults and rc.2 permission-UX fixes.

## 2. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1640 total, 0 failed, 13 ignored |
| PTY right-click test | right-click marker absent; Ctrl+V marker present |
| TUI interaction | pass (rc.3 log; quit/edit coverage unchanged) |

## 3. Known limits / blocked

- `intro-20` efficiency residual from the rc.2 Track Q run.
- CQ3/CQ6/CQ7/CQ9/CQ12/CQ16 remain Partial.
- Default network increases exposure to O1 (`browser_read` SSRF) and O2
  (journal hash chain).
- No third-party audit/certification, real Windows/macOS GA, signing,
  notarization, marketplace, mobile or R1–R9 metrics.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 4. Local assets

Built from tag `v1.0.0-rc.4` → `23d12cd` with `./scripts/package.sh
--linux-only`; tree clean, `reproducible=true`.

| Asset | Value |
|---|---|
| `1agent-1.0.0-rc.4-linux-x86_64.tar.gz` | SHA-256 `36cd694d7c965a92d8a73ebe80b377d0a2901aba1b9975cd19ff5abb966582ff` |
| Packaged binary | SHA-256 `971451e6622bdd9ee5eaf94f4bd7e52c9141719883b0de1b5269c54c26583bbb` |
| Provenance | `linux-x86_64.json`, commit `23d12cd`, `git_dirty: false` |
| Smoke | `sha256sum -c SHA256SUMS` OK; `--version` 1.0.0-rc.4; `doctor` Ok; install to `~/.local` OK |

Evidence: `docs/evidence/release-1.0.0-rc.4-2026-11-18.md`. No GitHub release
and no push were performed.
