# ADR 0102: Extract TUI Text Helpers

- Status: accepted
- Date: 2026-11-12
- Related: RFC 0001, SA97-25

## Context

RFC 0001 proposes splitting `tui.rs` by first extracting pure functions, then
transcript projection, input/command panels and finally state. `tui.rs` was
7,932 lines and mixed terminal state with text wrapping, sanitization and
width handling.

## Decision

1. Move the pure text helpers to `src/tui_text.rs`:
   `sanitize_inline`, `sanitize_multiline`, `sanitize_input`, `fit_row`,
   `wrap_display`, `split_display_width`, `truncate_to_width`.
2. Re-export them from `tui.rs` so existing callers and tests keep the same
   path. No behavior changes.
3. Move/duplicate focused unit tests into `tui_text.rs`; existing TUI tests
   continue to run through the re-exports.
4. This is the first SA97-25 slice. Transcript projection, input editing,
   command panel and state extraction remain.

## Consequences

- The pure helpers can be tested without a terminal and are independent of
  `TuiState`.
- `tui.rs` loses about 100 lines; the change is a move, not a rewrite.
- The TUI interaction contract is unchanged: the PTY check still passes.
- `main.rs` is untouched in this slice; its split remains future work.
