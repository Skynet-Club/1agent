# 1Agent 1.0.0-rc.5 — unified settings and themes (draft)

Date: 2026-11-18  
Status: **local code-level release candidate; not published, not GA**  
Scope: 1.0.0-rc.4 plus `/setting` and TUI themes

This is a working release note, not a GitHub release announcement. External
audit, real Windows/macOS GA, marketplace responsibility and R1–R9 user metrics
are **blocked** and must not be presented as passed.

## 1. What changed in rc.5

- **`/setting`** is the unified TUI settings entry; `/settings` is an alias and
  `/config` remains a compatibility entry.
- `/setting show` reports the model, network/write gates, read roots, budgets,
  language, workspace, theme and source.
- `/setting set` covers budgets, language, network/write and theme. Permission
  changes still apply to the next run only.
- **Eight themes**, TUI-only: military green (default), matrix green,
  monochrome, deep blue, sakura pink, copper orange, cyberpunk and wasteland.
  The theme is saved in `config.json` under `appearance.theme` and applied
  immediately without restart.
- **Two reset levels**: `/setting reset session` returns to startup values
  without writing the config; `/setting reset global` warns first, then requires
  `/setting reset global confirm`, backs up `config.json` and restores the
  full-local gate, network/write on and military green while preserving
  providers, keys, workspace and read roots.
- Config schema moves to **3** with optional `appearance`; v1/v2 configs still
  load. CLI help and non-TUI output keep the existing military-green palette.

## 2. Theme palette

| ID | Chinese | English | Direction |
|---|---|---|---|
| `military` | 军旅绿 | Military Green | default, existing palette |
| `matrix` | 矩阵绿 | Matrix Green | black + neon green |
| `mono` | 黑白灰 | Monochrome | grayscale, symbols carry semantics |
| `ocean` | 深蓝 | Deep Blue | navy + azure |
| `sakura` | 樱花粉 | Sakura | dark plum + pink accents |
| `copper` | 橙铜 | Copper Orange | warm brown + copper |
| `cyber` | 赛博风 | Cyberpunk | near-black purple + cyan/magenta |
| `wasteland` | 末日废土 | Wasteland | olive/rust, low saturation |

Themes are implemented as a post-render remap of the semantic base palette, so
new widgets inherit all themes without per-widget colour branches.

## 3. Verification (local)

| Check | Result |
|---|---|
| `./scripts/verify.sh` | pass; 1648 total, 0 failed, 13 ignored |
| TUI interaction | pass (real provider; two-Ctrl+Q quit) |
| Settings e2e | `/setting show`, `/settings theme matrix`, theme list pass |
| Theme tests | 8 unique themes, military identity mapping, non-default remap |
| Reset tests | session reset restores startup; global reset backs up and restores military/full-local |

## 4. Known limits / blocked

- Theme affects the TUI only; CLI `help` keeps military green by design.
- Themes are static; no custom theme file or theme editor yet.
- `intro-20` efficiency residual from the rc.2 Track Q run; CQ3/CQ6/CQ7/CQ9/
  CQ12/CQ16 remain Partial.
- No third-party audit/certification, real Windows/macOS GA, signing,
  notarization, marketplace, mobile or R1–R9 metrics.
- Development history contains an AI byline marker; a public release needs a
  clean release branch or an amended attribution policy.

## 5. Local assets

To be filled after the rc.5 package is built. No GitHub release and no push are
performed by the code-level candidate workflow.
