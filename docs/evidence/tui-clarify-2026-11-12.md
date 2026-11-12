# Clarify TUI Batch Panel — Evidence (2026-11-12)

Status: **local pass**
Scope: `/clarify <questions.json>`, 1–5 question batch, per-question lock,
`Other` editor, one confirm, cancel, 300-second expiry, result display.

## Captures

Real PTY frames from `scripts/pty_screenshot.py` (release binary, mock
provider):

```text
docs/evidence/tui-clarify-2026-11-12/panel-100x30.txt
docs/evidence/tui-clarify-2026-11-12/panel-72x20.txt
docs/evidence/tui-clarify-2026-11-12/locked-100x30.txt
docs/evidence/tui-clarify-2026-11-12/submitted-100x30.txt
```

- `panel-100x30` / `panel-72x20`: questions, recommended-first options,
  `Other…`, countdown and key hints.
- `locked-100x30`: both questions locked; footer switches to `Enter 提交`.
- `submitted-100x30`: transcript line
  `澄清结果：已提交 · 已回答 2 · 跳过 0 · 未回答 0 · 默认答案有效 是`.

## Behaviour

- `/clarify <path>` reads an array or `{questions:[...]}` (≤1 MiB), creates a
  `ClarifyPanel` (1–5 questions, at least one blocking) and opens a full-screen
  modal.
- Keys: `↑/↓` options, `←/→` or `Tab` questions, `Enter` lock/confirm,
  `S`/space skip, `Esc`/`Ctrl+C` cancel. `Other` opens an inline editor;
  `Enter` validates, `Esc` leaves it. Paste is single-line and capped at 500
  characters.
- A 500 ms timer calls `expire`; a timed-out round keeps missing questions
  unanswered and `default_answer_valid` true.
- The resolved round is stored in `TuiState::last_clarify`, displayed once in
  the transcript and written as JSON to `/details`.

## Model-initiated tool

The interactive TUI registers `ClarifyTool`; the model calls it with 1–5
questions and receives the resolved round as a JSON tool observation. The
tool timeout is 310 s, so the panel's 300 s expiry wins. Line and headless
modes do not register the tool.

A real PTY run asked the model to call `clarify` once, answered the panel, and
the model continued to a final answer. Capture:
`docs/evidence/tui-clarify-tool-2026-11-12.txt`.

## Validation

```bash
cargo test --lib --offline tui_clarify
cargo test --lib --offline tui::tests::clarify
cargo test --offline
cargo clippy --all-targets --offline -- -D warnings
cargo fmt --check
```

- `tui_clarify`: 8 passed.
- TUI panel tests: 4 passed.
- Full suite: **1254 passed / 0 failed / 13 ignored**.
- Clippy: 0 warnings; `cargo fmt --check`: clean.

## What this does not prove

- Model-initiated clarify through a runtime tool is not wired; the command is
  user-initiated.
- Question history is not persisted across sessions.
- Mouse interaction, IME composition windows and every terminal's key
  encoding are not covered; the `Other` editor is covered by a `TestBackend`
  unit test rather than the PTY capture.
- A real 300-second wait is exercised by unit tests with an injected clock,
  not by a five-minute wall-clock run.
