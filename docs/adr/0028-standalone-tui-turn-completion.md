# ADR 0028: Standalone TUI Turn Completion

- Status: accepted for the interaction slice
- Date: 2026-11-08

## Context

The end-of-turn next-action menu was opened after every TUI run, including a
standalone 1Agent greeting such as "你好，模型连接测试". The modal panel covered
the answer and defaulted to `/frame`, so a simple conversation looked like a
command prompt for a task the user had not requested. The default mock provider
also called `search_files` for a connectivity greeting because tools were
advertised without a small-talk rule.

## Decision

1. The automatic next-action panel is only shown for non-standalone hosts
   (Workshop master/worker). A standalone turn ends with the answer and the
   normal input line; slash commands remain available through the `/` drawer.
2. The standalone status line includes `/ 命令` so the command surface remains
   discoverable without a modal.
3. For a stage without a mission, the preference order is `status`, `evidence`,
   `frame`; `/frame` is no longer the default.
4. The system prompt tells the model not to call tools for greetings,
   connectivity checks or purely conversational turns.
5. Because many terminals do not report Ctrl+Tab separately, the mode menu is
   also bound to Ctrl+T and Shift+Tab, and a `/mode` command opens the same menu
   from the drawer. The status line advertises Ctrl+T.
6. A PTY-level interaction check (`scripts/tui-interaction-check.sh`) renders a
   standalone greeting, `/help`, `/status`, `/mode` and Ctrl+T, and fails if the greeting opens
   the next-action menu or the command panels do not render.

## Consequences

- Standalone 1Agent behaves like a conversation, not a command menu; Workshop
  hosts retain the explicit next-step navigation.
- Users discover commands through `/` instead of an unsolicited panel.
- The interaction check requires the pyte venv; it is skipped with a notice when
  that interpreter is unavailable, and runs in `release-check.sh` after the
  release binary is rebuilt.
