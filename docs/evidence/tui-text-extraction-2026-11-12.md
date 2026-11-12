# TUI Text Extraction — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: first RFC-0001/SA97-25 split slice.

## Change

- New `src/tui_text.rs` (154 lines) holds `sanitize_inline`,
  `sanitize_multiline`, `sanitize_input`, `fit_row`, `wrap_display`,
  `split_display_width` and `truncate_to_width`.
- `src/tui.rs` re-exports them; its size drops from 7,932 to 7,828 lines.
- No behavior or public-path change for existing callers.

## Commands and results

```text
cargo test tui_text --lib          -> 5 passed
cargo test --lib tui::             -> 71 passed, 1 ignored
./scripts/tui-interaction-check.sh -> passed
  pty-backspace: passed
  pty-edit-stability: passed (400 mixed edit events)
  drawer/help/status/mode/Tab/completion/confirmation/paste: passed
```

## What this proves

- The moved helpers keep their wrapping, sanitization, padding and truncation
  behavior, including wide characters.
- The TUI interaction contract is unchanged under the same PTY suite.
- `tui.rs` can shrink while keeping its existing call sites and tests.

## What this does not prove

- That the rest of `tui.rs` is split; transcript, input, command panel and
  state remain.
- That `main.rs` is split at all.
- Any performance or behavior improvement; this is a move-only slice.
