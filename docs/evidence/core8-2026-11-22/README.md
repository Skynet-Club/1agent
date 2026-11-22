# Core 8 tool surface live check (2026-11-22)

Goal: cut the default TUI surface from 14 core tools to 8, leaving the other
tools behind `request_tools`, and measure the fixed per-call cost.

## Change

`CORE_TOOL_NAMES` is now:

`read_file`, `write_file`, `edit_file`, `search_files`,
`terminal_create`, `terminal_exec`, `preview_file`, `acceptance_gate`.

With the virtual `describe_self` and `request_tools`, the TUI advertises 10
tools instead of 16. `browser`, `clarify`, `verify_artifact`, `read_document`,
`skill_search` and `skill_list` are available only through `request_tools`.
The page delta now tells the model to request `preview_file`, `browser` or
`clarify` when they are missing.

## Measurement

Both runs: installed binary `cfcca216…`, real DeepSeek, TUI through tmux.

### Note task

Workspace `/tmp/tui-core8-test`, session `ses_tui_core8_20261122`, journal
`e64e9bc2-2c45-4064-8ecf-4b57ef2ea61e`.

Task: `Execute: create /tmp/tui-core8-test/note.txt with exactly the text: core8`

Result: `completed`, 6 model calls / 5 tool calls, file `core8`.

| Metric | 16 tools (`0dcb6885`) | 10 tools (`e64e9bc2`) |
|---|---|---|
| Advertised tools | 16 | **10** |
| System tokens (step 0) | 5,168 | 3,862 |
| Tool schema tokens | 3,099 | **1,910** |
| Fixed system + schema | 8,267 | **5,772** |
| Context tokens | 8,487 | **5,993** |
| Input tokens | 8,729 | **6,215** |

The advertised set was exactly: `acceptance_gate`, `describe_self`,
`edit_file`, `preview_file`, `read_file`, `request_tools`, `search_files`,
`terminal_create`, `terminal_exec`, `write_file`.

### Page task

Workspace `/tmp/tui-core8-page`, session `ses_tui_core8_page_20261122`,
journal `f3e34269-1f94-429d-97fc-54fb43401a6d`.

Objective: single-file landing page for 云笺, audience independent developers,
primary CTA 开始使用, written to `/tmp/tui-core8-page/index.html`.

The page delta and checklist were sent; the first request still had 10 tools
and no browser. The model called `request_tools` for `browser`, `unslop` and
`web-ui-audit`; `browser` was granted, the two unknown design-review tools
were denied with `unknown_tool`. The model then ran preview and two browser
audits (`3 viewports · overflow 0 · placeholders 0 · small tap targets 0`)
and called `acceptance_gate`. 15 model calls, 89.8s wall, stop `completed`,
answer `conditional`: the post-fix screenshots had the same sha256 as the
pre-fix screenshots, so the visual evidence was treated as cached. The page
itself is 11,027 bytes and the non-visual checks passed.

## Tests

- `core_tools_reduce_the_advertised_surface`: advertised set includes
  read_file/terminal_exec/acceptance_gate, excludes browser/clarify/web_search/
  spawn_agent, at most 10 tools, tool schema at most 2,000 tokens.
- `core_tools_can_be_requested_back`: `request_tools` still adds an excluded
  tool to the next model request.
- `act_prompt_sends_only_the_deltas_its_tools_can_use`: note Act omits the
  page/capability/followup/answer-style blocks; page Act includes the page
  block.
- Gates: lib 1423/0/4, bin suite exit 0, fmt, strict clippy, eval 8/8.

## Limits

- `browser`, `clarify`, `verify_artifact`, document and skill tools now cost a
  `request_tools` turn when a task needs them. The page run shows the recovery
  works, but it is an extra model step.
- The page answer is conditional for a real screenshot-cache doubt, not
  because the reduced surface failed.
- The model asked for `unslop`/`web-ui-audit`, which are described in the
  prompt but are not registered tools. That gap predates this change.
- CLI `run` still advertises the full surface.
- Cumulative fixed cost: 11,914 (38 tools) → 8,267 (16 tools) → 5,772
  (10 tools); first context 12,287 → 8,487 → 5,993.
