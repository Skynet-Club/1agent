# TUI Unslop Pass — Evidence (2026-11-12)

Status: **local pass (P0 + safe P1)**
Scope: contrast, splash footprint, status/metrics noise, one accent/link role,
help position indicator, empty-composer hint.
Based on unslop by Damir Sokolovsky (@heydamir), CC BY 4.0.

## Captures

Before and after frames were rendered through a real PTY with
`scripts/pty_screenshot.py` (release binary, mock provider):

```text
docs/evidence/tui-unslop-2026-11-12/ready-100x30-before.txt
docs/evidence/tui-unslop-2026-11-12/ready-100x30-after.txt
docs/evidence/tui-unslop-2026-11-12/ready-72x20-before.txt
docs/evidence/tui-unslop-2026-11-12/ready-72x20-after.txt
docs/evidence/tui-unslop-2026-11-12/help-100x30-before.txt
docs/evidence/tui-unslop-2026-11-12/help-100x30-after.txt
docs/evidence/tui-unslop-2026-11-12/task-100x30-before.txt
docs/evidence/tui-unslop-2026-11-12/task-100x30-after.txt
```

## Changes

- Body/work-log text green `#487748` → `#5E9E5E`; border green `#3F644B` →
  `#4A7855`; local-file links and the accent/language label use `#3FB950`
  instead of deep green and ANSI blue 75. The conclusion border uses the
  border green.
- The full ASCII mark now requires enough transcript height; a 72×20 terminal
  gets the compact `1-Agent` wordmark in the brand green. The compact mark no
  longer uses blue.
- The idle top caption is `就绪` only; the bottom status line no longer repeats
  `就绪` and shows `default · 直接回答 · Ctrl+T 模式`.
- Missing token/cache metrics are omitted instead of rendered as `↑ — / ↓ — ·
  缓存 —`.
- `/help` shows the selected position, for example `1/29`.
- An empty composer shows `输入任务，/ 查看命令`; the hint disappears on input.

## Contrast

| Pair | Before | After |
|---|---|---|
| Body green on `#06100A` | 3.69:1 (`#487748`) | 6.01:1 (`#5E9E5E`) |
| Body green on `#08140D` | 3.59:1 | 5.85:1 |
| Border green on `#06100A` | 2.89:1 (`#3F644B`) | 3.78:1 (`#4A7855`) |
| Local-file link on `#06100A` | 2.01:1 (`#1F4E28`) | 7.61:1 (`#3FB950`) |

## Validation

```bash
cargo test --lib --offline tui::tests
cargo test --offline
cargo clippy --all-targets --offline -- -D warnings
cargo fmt --check
```

- TUI unit tests: 71 passed / 0 failed / 1 ignored.
- Full suite: **1242 passed / 0 failed / 13 ignored**.
- Clippy: 0 warnings; `cargo fmt --check`: clean.
- The four PTY captures above are text frames; colors are asserted by
  `TestBackend` unit tests, not by the text capture.

## Environment note

The first full-suite run failed `cli_tui_agents_refresh_uses_configured_profiles`
because the local skynet-term registry held 70 agents (67 expired) and the TUI
rejects more than 64 after merging. The 67 expired records were moved to
`~/.local/state/skynet-term/agents-stale-20261112/` and the test passed twice.
The underlying robustness issue (stale agents count toward the TUI limit)
remains a follow-up, not part of this pass.

## Deferred

- Bilingual chrome labels (`就绪/Ready`, `任务理解/Task Understanding`,
  `结论/Conclusion`) need a locale decision; not changed.
- Help command grouping beyond the position indicator.
- Prose width cap on very wide terminals.
- Stale-agent filtering in `/agents refresh`.
