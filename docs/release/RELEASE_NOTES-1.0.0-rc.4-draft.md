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

To be filled after the rc.4 package is built. No GitHub release and no push are
performed by the code-level candidate workflow.
