# ADR 0127: TUI Work-Log Pairing and Conditional Answer Style

- Status: accepted
- Date: 2026-11-13
- Related: M19–M23, M21/M22

## Context

A real TUI transcript exposed four problems:

- every model request appeared twice in the work log, with identical message
  and context counts;
- the request metrics were a separate result row, so each step consumed two
  rows before any tool ran;
- parallel tool results were appended after all calls, so a result was not
  visibly paired with its call;
- the final answer used a four-column table with truncated cells and ended
  with an unnecessary A/B/C question for a direct question.

The duplicate came from `append_work_events`: the model-request branch used
`continue` before advancing `last_seq`, so the next journal event replayed and
appended the same request row.

## Decision

1. Advance `last_seq` before the model-request branch continues. A request row
   is emitted once per journal event.
2. Use a distinct `Model Request` work kind and one row per step:
   `第 N 步 · M 条消息 · T tokens`. Do not emit a second metrics row.
3. Store the tool `call_id` on the work entry. When the observation arrives,
   insert its result immediately after that call entry, so parallel calls keep
   call/result pairs adjacent.
4. Prefix results with the tool name and humanize common failures
   (`网络请求失败（URL）`, `请求超时`, `HTTP <code>`); raw details remain in
   `/details`. Localize stop reasons (`完成`, `预算耗尽`, `已取消`, ...).
5. If a Markdown table would truncate a cell or has more than four columns,
   render each row as a stacked label plus `header：cell` lines instead of
   clipping.
6. The answer prompt asks a question only when the next action depends on the
   user's choice; alternatives only when the user is choosing. Tables are for
   three or more items across two or more dimensions, at most three columns.

## Consequences

- The work log is one request row per step, with adjacent call/result pairs and
  fewer internal metrics.
- Very wide tables become longer but readable; simple comparisons stay prose or
  bullets.
- The prompt change reduces unnecessary follow-ups but does not guarantee model
  compliance; M22 still needs the blind-review sample.
- The duplicate-row regression is covered by a rendered-buffer test; call/result
  ordering and the stacked-table fallback have unit tests.
