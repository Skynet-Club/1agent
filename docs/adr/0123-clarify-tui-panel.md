# ADR 0123: Clarify TUI Batch Panel

- Status: accepted
- Date: 2026-11-12
- Related: SA97-35

## Context

ADR 0096 defined the clarify gate and `ClarifyPanel` state machine but left
rendering and terminal input to a frontend. The 0.97 plan requires a blocking
1–5 question batch with per-question lock, `Other` text, one confirm, cancel
and a 300-second timeout. Without a TUI adapter the panel could only be driven
by the CLI round resolver.

## Decision

1. Add `tui_clarify::TuiClarifyState`. It owns focus, option selection,
   `Other` text, error display and expiry; all validation stays in
   `ClarifyPanel`.
2. Add the Core command `/clarify <questions.json>`. It reads an array or an
   object with `questions` (≤1 MiB, ≤50 questions), then creates a
   `ClarifyPanel` (1–5 questions, at least one blocking). Invalid panels fail
   with a transcript notice.
3. The panel is a full-screen modal and owns the keyboard while open:
   `↑/↓` moves options, `←/→` or `Tab` moves questions, `Enter` locks the
   selected option or confirms once every question is resolved, `S`/space
   skips, `Esc` cancels, and `Ctrl+C` cancels. Selecting `Other` opens an
   inline editor; `Enter` validates and locks, `Esc` leaves the editor.
4. The panel polls expiry every 500 ms while open and calls
   `ClarifyPanel::expire`. Timeout leaves missing questions unanswered and
   keeps `default_answer_valid` true.
5. A finished round is stored in `TuiState::last_clarify`, shown as one
   transcript line and written to `/details` as JSON. For a model-initiated
   call it is also sent back through the `clarify` tool's oneshot channel.
6. Line mode reports that the panel needs an interactive terminal instead of
   opening a dead panel.
7. Register `ClarifyTool` only in the interactive TUI. The tool validates
   1–5 questions, forwards them to the panel and waits up to 310 seconds; it
   returns the summary and per-question answers as a JSON tool observation.
   Cancellation closes the panel with `cancelled`; the 300-second panel
   deadline returns `timed_out`.

## Consequences

- The same panel state machine now has a real terminal frontend; the CLI and
  TUI share the same outcome vocabulary and round resolution.
- Model-initiated clarify works in the interactive TUI; line and headless
  modes do not register the tool and cannot block on a panel.
- The panel is keyboard-only. Mouse selection and persisted question history
  remain future work.
- The new command increases the standalone Core command count from 29 to 30;
  command-count tests and `docs/tui.md` are updated.
