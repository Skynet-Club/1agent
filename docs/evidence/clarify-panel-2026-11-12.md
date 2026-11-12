# Clarify Batch Panel State Machine — Evidence (2026-11-12)

Status: **state machine passed on this machine**
Scope: selection, locking, confirmation, cancellation and timeout.

## Commands

```bash
cargo test --offline clarify --lib
cargo test --offline --test clarify_cli
```

## What this proves

- Two blocking questions can be answered and confirmed in one round
  (`submitted`, answered=2, unanswered=0, default valid).
- A second selection for the same question fails with `DuplicateAnswer`; a
  closed panel rejects further selection with `PanelClosed`.
- The `Other` option requires non-empty text and stores trimmed text; a
  non-Other option with text fails closed; an unknown option fails closed.
- An all-skipped confirmation becomes `cancelled`, not an empty submission.
- `expire` returns `None` before 300 seconds and `timed_out` at the deadline;
  missing questions remain unanswered and the default stays valid.
- The panel rejects empty batches and batches with no blocking question.

## What this does not prove

- TUI rendering, key handling or a real 300-second wall-clock block.
- Persistent question history or cross-session deduplication.
- That a model will ask good questions or that the recommended option is the
  right default for the task.
- That `Other` text is passed to the executor; the panel stores it, but the
  round summary does not.
