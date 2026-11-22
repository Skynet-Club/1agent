# Compact system prompt live check (2026-11-22)

Goal: cut the fixed system prompt from ~3.9k to ~3.2k tokens without deleting
behavior. The slice combines wording compression with finer instruction
deltas.

## Change

- Moved the code-block rule, replay hint and read-only action rule to
  `AGENT_PROMPT_ANSWER_STYLE`; Act runs no longer pay for them.
- Added `AGENT_PROMPT_SKILLS`, sent only when `skill_search` or `skill_list`
  is advertised.
- Split the old follow-up block into three deltas: `prompt_library` (when
  known facts exist), `memory` (only when known facts are not prompt-library
  sources), and `followup` (only with prior conversation turns or memory
  records).
- Compressed the super-agent discipline, working style, formatting, claims,
  permissions, understanding, clarify, task_need, delivery, write/verify,
  conflicts, acceptance and task-profile wording while preserving every
  asserted rule.
- `prompt_budgets` gates: core < 2,500, Answer < 2,800, Act note < 3,100.

## Live check

Both runs: installed binary, real DeepSeek, TUI through tmux, same note task
shape.

### Stage A — before the memory/follow-up split (binary `51914d89…`)

Workspace `/tmp/tui-prompt-compact`, session `ses_tui_prompt_compact_20261122`,
journal `fb846fa4-…`.

The only known fact was `Prompt library sources: …`, but the old follow-up
delta was sent in full. First system message: 13,639 characters / **3,421
tokens**. Step 0: system 3,715, schema 1,543, fixed **5,258**, context 5,508,
input 5,784. 6 models / 5 tools, completed.

### Stage B — after the split (binary `c1d1114a…`)

Workspace `/tmp/tui-prompt-split`, session `ses_tui_prompt_split_20261122`,
journal `8903ff4f-…`.

First system message: 12,419 characters / **3,116 tokens** — under the 3.2k
target. Step 0: system **3,409**, schema 1,543, fixed **4,952**, context
**5,201**, input **5,530**. 6 models / 4 tools, completed.

The live step-0 system message contains the artifacts delta and the prompt
library line, and contains none of: page, capability, skills, follow-up,
cross-session memory, or Answer style.

| Metric | Accept compact | Stage A | Stage B |
|---|---|---|---|
| First system message | 3,688 | 3,421 | **3,116** |
| Step-0 system | 3,864 | 3,715 | **3,409** |
| Tool schema | 1,543 | 1,543 | 1,543 |
| Fixed system+schema | 5,407 | 5,258 | **4,952** |
| Context | 5,632 | 5,508 | **5,201** |

Cumulative from the 38-tool baseline: fixed 11,914 → **4,952** (-58%), first
context 12,287 → **5,201** (-58%).

## Tests

- `prompt_deltas_are_added_only_when_requested` now also checks that a
  prompt-library-only delta does not pull in the follow-up block, and that the
  follow-up block does not pull in cross-session memory.
- `prompt_budgets` enforces the new core/Answer/Act budgets.
- The full prompt-contract suite still passes: every asserted rule is still
  present in `agent_system_prompt()`.
- Gates: lib 1428/0/4, bin suite exit 0, fmt, strict clippy, eval 8/8.

## Limits

- One live note run per stage; no live page or code-display task after the
  rewrite. The page delta is unchanged and the code rule is still present for
  Answer/Ask, but the shorter wording may be interpreted differently.
- Stage B's fixed 4,952 is above the 4.7k estimate because step-0 system also
  includes the task profile, environment manifest and ledger context. The
  first model system message itself is 3,116.
- CLI `run` still has the full tool surface, so it includes the capability and
  skills deltas.
- Wording was compressed, not removed; subtle behavior changes are possible.
