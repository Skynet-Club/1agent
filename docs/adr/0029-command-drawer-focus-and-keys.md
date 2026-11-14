# ADR 0029: Command Drawer, Focus and Arrow-Key Semantics

- Status: accepted for the interaction slice
- Date: 2026-11-08

## Context

The slash drawer only matched command-name prefixes, hid stage/running-disabled
commands, closed as soon as an argument was typed, and Enter could silently
complete and execute a partial command. Escape cleared the whole input. Command
panels had no horizontal-key semantics, and Enter on a partial command could
submit something the user had not selected.

## Decision

1. `command_entries` keeps Master commands hidden outside a Workshop master, but
   shows stage-gated and running-disabled commands with a reason. Scope is still
   filtered; reason is rendered as `[Approval]`, `[运行中不可用]`, etc.
2. Drawer search scores exact name, name prefix, name substring and (for queries
   of at least three ASCII characters or any non-ASCII query) description
   substring. Registry order is preserved within a score.
3. `Tab` completes the selected command. Commands with arguments get a trailing
   space and a usage hint such as `/frame <请求或当前任务>`.
4. `Enter` on an incomplete command only completes it; a second Enter executes
   the exact command. Disabled selections show the reason and do not submit.
5. `Esc` dismisses the drawer but keeps the typed input; further edits reopen it.
6. `Ctrl+K` opens the drawer with `/`.
7. Empty-input Up/Down recalls submission history; text editing, panels and
   drawers keep their existing selection/scroll behavior.
8. In the mode picker, Left closes and Right applies. In a command panel, Left
   returns to the parent panel or closes, and Right is equivalent to Enter.
   Composer Left/Right continue to move the text cursor.
9. The empty `/` query groups commands under `最近`, `会话`, `思考`, `证据`,
   `模型与权限` and `总控`. The three most recently accepted commands lead the
   list and are removed from their category so a row is never duplicated. A
   non-empty query keeps the flat fuzzy-match order; recency is process-local.
10. Destructive commands (`/clear`, non-dry-run `/dispatch`, `/control cancel`
   and `/control takeover`) arm a confirmation notice and execute only on a
   second Enter. Any other input except Esc and Ctrl+Q — typing, navigation,
   mouse or opening the command drawer — cancels the pending command; Esc
   reports the cancellation. The original `/session delete`
   double-confirmation syntax is unchanged.
11. While a command panel is open it owns the keyboard: character, delete, Tab,
   Home/End and paste input is ignored rather than editing the hidden composer.
   Prompt menu items close the panel before placing a command template in the
   composer, so input is possible only after focus returns.
12. PTY checks cover a greeting without a modal next menu, `/help`, `/status`,
   `/mode`, Ctrl+T, categorized drawer, Tab argument completion, destructive
   confirmation and panel focus isolation.

## Consequences

- The drawer explains what exists but is unavailable instead of silently hiding
  it; completion no longer executes a command by accident.
- Read-only output commands (`/status`, `/journal`, `/evidence`, `/audit`,
  `/report`, `/tools`, `/cost`, `/copy`) now append to the transcript instead of
  replacing it. Pickers and hierarchical panels still use the command panel.
- Nested command panels render a breadcrumb such as
  `/workspace › 设置工作区`.
- A panel no longer leaks keystrokes into an invisible composer; returning to
  the composer requires closing the panel or choosing a prompt item.
- Mouse selection, a preview pane and the full picker/form split are not
  implemented yet. Recency does not persist across processes.
