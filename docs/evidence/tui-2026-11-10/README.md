# TUI activity card and editing stability — 2026-11-10

## Input panel

The whole TUI sits on a dark-green backdrop `#06100A`, 80% of the input panel background
brightness and slightly darker (asserted by
`terminal_backdrop_is_darker_green_than_the_input_panel`). The session
input box is a dark-green panel: background `#08140D`, border `#3F644B`, panel text
`#487748` (the former bright palette dimmed by 50%); input text stays white.
While the width allows, token metrics are embedded in the right side of the top border:

```text
╭─ • 就绪 · Ready ────────── • ↑ — / ↓ — · 缓存 — · 上下文 ──────────────────── 0/1.0M 0% · 0s ────╮
│                                                                                                  │
│›                                                                                                 │
│                                                                                                  │
╰─ 就绪 · default · 直接回答 · Ctrl+T 模式  ───────────────────────────────────────────────────────╯
```

On a narrow terminal the heading stays on the top border and the metrics fold to one
row inside the same fence:

```text
╭─ • 就绪 · Ready ─────────────────────────────────────────────────────╮
│ ↑ — / ↓ — · 缓存 — · 上下文 ────────── 0/1.0M 0% · 0s                │
│                                                                      │
│›                                                                     │
│                                                                      │
╰─ 就绪 · default · 直接回答 · Ctrl+T 模式  ───────────────────────────╯
```

See `input-panel.txt` (100 columns) and `input-panel-folded.txt` (72 columns) for the two
real PTY layouts. `activity-card.txt` preserves the earlier two-line card
capture. `narrow_terminal_folds_metrics_inside_the_same_fence` asserts the folded row.

The top title is derived from the run journal, not a timer: `ModelRequested` becomes
`模型思考 Thinking` with the step count, `ToolCalled` becomes a bilingual
reading/writing/creating/updating/searching/fetching/querying/Bash/subagent
state with a safe path/query detail, `ToolObserved` becomes `工具结果 Tool
Result`, and `ProviderRetryScheduled`/`ContextCompacted`/`AnswerProduced` have
their own labels. The panel title, embedded metrics and bottom border use the dimmed panel green on a
dark green surface; input text stays white. User messages render on the panel background
`#08140D` with white text (asserted by
`user_message_and_answer_are_separated_by_an_empty_gap_row`). The work log uses deep
green text, a bright-green bullet and bright-green links. The
context meter uses `─` cells matching the fence border, 20 on wide terminals and 10 when
the panel folds: dimmed panel green for used cells, black for the remaining cells.
Master-Agent states use the input-panel title and embedded footer
with `任务理解 Mission Understanding`, `任务分配 Task Assignment`, `任务启动
Task Starting`, `Worker 运行 Worker Running`, `任务验收 Mission Verification`
and `汇报 Reporting`. See `input-panel.txt` for the merged input panel.

The same journal events are also appended to the main transcript as a visible
work log, so tool calls and results are not hidden in the status strip:

```text
 • 任务理解 Task Understanding · 解析目标与约束
 • 规划 Planning · 任务计划已生成
 • 模型思考 Thinking · 第 1 步
   ⎿ 请求 2 条消息 · 上下文 3775 tokens · 工具 22 个
   ⎿ 返回工具调用 · read_file
 • 读取 Reading · read_file · /home/skynet/1-agent/README.md
   ⎿ read README.md (999 bytes)
 • 模型思考 Thinking · 第 2 步
   ⎿ 请求 4 条消息 · 上下文 7026 tokens · 工具 22 个
   ⎿ 返回最终答案 · 输出 0 tokens · ctrl+o 详情
```

See `work-log.txt` for a real PTY capture. The raw `work-log.txt` and
`activity-card.txt` files predate the `第 N 步` label and the `⎿` request/response
summaries; their `第 1/6 步` denominator was the old display of the `--max-steps`
budget. Current thinking steps print request and response summaries, and `Ctrl+O`
opens the run details and process trace.
Bash results include a bounded
output preview; file paths, search queries and subagent tasks are shown with
their tool names. The state and detail are separated by a single ` · `; the
state name, arguments and `⎿` results use deep green `#487748`, none of them
bolded, and every `•` is bright green (asserted by
`work_log_uses_deep_green_text_with_bright_green_marker`). Local file paths and
http(s) URLs are both bright green (asserted by
`work_log_colors_links_bright_green`). A real-PTY sample of a deterministic
`@read-file:` run showed `•` at `00ff00`, the label and `⎿` at `487748`, and
both the argument path and the result `README.md` at `00ff00` on `06100a`; the
user message row background was `08140d`.

## Final answer fence

The final answer is no longer loose transcript text. Direct and structured
answers share a deep-green `结论 Conclusion` fence with a bright-green, bold title
and white, unbolded content:

```text
╭─ 结论 Conclusion ─────────────────────────────────────────────╮

   不用启动，GridCraft 0.16.0 此刻已经在运行。

╰───────────────────────────────────────────────────────────────╯
```

## Prompt drawer

The `/` drawer also searches the 48 embedded prompt supplements by name, category,
trigger or purpose. The empty drawer starts with three featured prompts. Selecting one
fills the composer with its template and leaves `{{variables}}` for editing; it never
executes automatically. After a task, a trailing ` //` (for example
`做跨境电商 //验收`) opens a prompt-only drawer; selecting merges the template before
or after the task, and `Esc` restores the task.

```text
       ╭ 命令 ──────────────────────────────────────────────────────────────────────────────╮
       │  提示词补充                                                                        │
       │▌ 先说验收标准          A 必放 · 加目标 · 动手前先定义「好」长什么样                │
       │  每条给验收方式        A- 推荐 · 加执行 · 让产出自带质检                           │
       │  变成可勾选清单        A 必放 · 加执行 · 把结论变成待办                            │
       │  /frame                整理事实、假设、约束、未知和验收标准                        │
       ╰──────────────────────────────── ↑↓ 选择 · Tab 补全 · 输入关键词搜提示词 · Esc 关闭 ╯
```

After Enter:

```text
╭─ • 就绪 · Ready ───────────────────────────────────────────── • ✓ 已插入提示词：先说验收标准 ────╮
│                                                                                                  │
│› 先别动手。用三句话写出这个任务「做得好」的验收标准，等我确认之后再开始做。                      │
│                                                                                                  │
╰─ 就绪 · default · 直接回答 · Ctrl+T 模式  ───────────────────────────────────────────────────────╯
```

See `prompt-drawer-empty.txt` (empty `/`, three featured prompts),
`prompt-drawer-task.txt` (task + ` //`, prompt-only drawer) and
`prompt-drawer.txt` / `prompt-insert.txt` (search + insertion) for the real PTY
captures.

The structured reasoning sections that follow the fence (主张 / 依据 / 假设 / 约束 /
未知 / 冲突 / 翻转条件) render entirely in deep green `#487748` — headings, labels,
bullets and body — while only the answer above stays white (asserted by
`structured_sections_are_deep_green_while_the_answer_stays_white`).

See `conclusion-fence.txt` for a real PTY capture; the rendered border colour,
bright-green bold title and white body are asserted by
`final_answer_is_wrapped_in_a_conclusion_fence`. Real PTY sampling showed the
`结` title cell at `00ff00` bold, the bottom-border `T` at `00ff00` bold, and
`/help` drawer names at `00ff00` bold with descriptions at `00ff00` unbolded.

## Editing and selection

- `cargo test --offline --lib tui::tests` covers grapheme-aware editing, mouse
  drag selection in the composer (including the whole composer as a hit target),
  and 2500 deterministic mixed edit operations.
- The `/` drawer now includes the 48-entry prompt library; real PTY `/验收` showed
  `先 说 验 收 标 准` under `提示 词 补 充`, and Enter filled the composer with
  `先 别 动 手。…验收 标 准…` without executing it.
- Real PTY sampling of the green overlays: `/details` border `00ff00` on surface
  `0a1f12`; the mode menu uses `0a1f12`/`143d24` with bright-green labels; the `/`
  drawer uses `0a1f12` with bright-green command names (asserted by
  `interaction_overlays_use_green_surfaces`).
- A real PTY right-click outside the composer produced no paste; the same
  right-click inside the composer showed `已从剪贴板粘贴` (asserted by
  `right_click_paste_is_limited_to_the_composer`).
- `python3 scripts/pty_edit_stability.py` drives the release TUI through 400
  real PTY events: bracketed multi-line/unicode paste, backspace, spaces, arrows,
  Delete, mouse drag and short turns. It passed in the release check.
- `python3 scripts/pty_screenshot.py --cols 72 --rows 20` produced
  `input-panel-folded.txt`; the same script at `--cols 100` keeps the metrics on
  the top border.
- `python3 scripts/pty_backspace.py` continues to pass.

## Crash diagnostics

The TUI now installs a panic hook that restores the terminal and appends to
`$HOME/.1a/tui-crash.log`; transient input-reader errors are retried. A future
crash report should include that log and the edit/mouse sequence.
