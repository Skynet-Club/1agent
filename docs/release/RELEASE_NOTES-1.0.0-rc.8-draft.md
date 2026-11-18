# 1Agent 1.0.0-rc.8 — path-vs-command fix (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.7 plus the TUI path parsing fix

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What changed in rc.8

- A local absolute path entered as a task no longer becomes `Unknown command`.
  The TUI resolver now treats an unknown `/...` input as a task when it is an
  existing path or a multi-segment absolute path. A single unknown slash word
  still reports `Unknown command`, so typo feedback remains.
- `//` remains the prompt-drawer trigger and is not treated as a path.
- Example: typing `/home/skynet/文档/Skynet-Docs/...md` now submits it as a
  task instead of failing in the command parser.

## 2. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1652 total, 0 failed, 13 ignored |
| claims table | well-formed; 261 checked (237 verified / 16 partial / 8 no evidence) |
| Path resolver test | absolute existing path and missing multi-segment path → task; single unknown slash → Unknown command |
| Includes | rc.7 repeat/pass^k evaluation, rc.6 settings panel, rc.3 full-power defaults |

## 3. Known limits / blocked

- Path detection is shape-based; a single-segment nonexistent path such as
  `/newfile` is still treated as an unknown command.
- Human calibration, 8h DeepSeek quality, external audit, real Windows/macOS
  GA, marketplace and R1–R9 remain blocked.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 4. Local assets

To be filled after the rc.8 package is built. No GitHub release and no push are
performed by the code-level candidate workflow.
