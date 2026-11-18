# 1Agent 1.0.0-rc.6 — settings panel UX (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.5 plus the hierarchical settings panel

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What changed in rc.6

- `/setting` now opens a **two-level panel** instead of a flat list that
  overflowed the visible height.
- First level: current settings, run budget, permissions, appearance, model
  and reasoning, reset, close.
- Appearance is a second-level page with the eight themes; selecting a theme
  applies immediately and stays in the panel.
- Numeric values are edited **inside the panel**: Enter saves, Esc cancels, and
  no command is injected back into the composer.
- Permissions are toggled in-panel; read roots can be added or removed without
  leaving the panel.
- Global reset opens a dedicated warning/confirmation page; `confirm` is a
  menu item, not a command typed into the conversation.
- The command panel now uses the available height for the menu instead of a
  hard cap of eight rows.

## 2. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1649 total, 0 failed, 13 ignored |
| TUI interaction | pass (real provider; two-Ctrl+Q quit) |
| Settings panel PTY | `/setting` → budget → inline edit → `已更新设置：steps` |
| Settings e2e | `/setting show`, `/settings theme matrix`, theme list pass |
| Reset tests | session reset restores startup; global reset backs up and restores military/full-local |

## 3. Known limits / blocked

- The setting panel is a command panel, not a full-screen settings page.
- Theme affects the TUI only; CLI `help` keeps military green by design.
- Themes are static; no custom theme file or editor yet.
- `intro-20` efficiency residual from the rc.2 Track Q run; CQ3/CQ6/CQ7/CQ9/
  CQ12/CQ16 remain Partial.
- No third-party audit/certification, real Windows/macOS GA, signing,
  notarization, marketplace, mobile or R1–R9 metrics.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 4. Local assets

To be filled after the rc.6 package is built. No GitHub release and no push are
performed by the code-level candidate workflow.
