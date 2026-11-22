# Instruction deltas live check (2026-11-22)

Goal: stop sending every agent rule on every call. The prompt is now a core
block plus five optional blocks selected from the run's advertised tools and
existing task checks.

## Change

`agent_system_prompt_for_run` composes `AGENT_PROMPT_CORE` with:

| Delta | Sent when |
|---|---|
| artifacts | `write_file`, `edit_file` or `acceptance_gate` is advertised |
| capability | `capability_search` or `resolve_capability` is advertised |
| page | artifacts are on and the existing `looks_like_page_task` matches |
| followup | session context or `known_facts` is non-empty |
| answer_style | interaction mode is Answer or Ask, not Act |

`agent_system_prompt()` remains the full prompt (all deltas) for provider
probes and the prompt-contract tests. No new keyword gate was added; the page
delta reuses the detector that already decides when to inject the page
checklist.

Deterministic composed sizes (`estimate_tokens`, includes the JSON schema):

| Composition | Tokens |
|---|---|
| core only | 2,910 |
| core + answer_style | 3,092 |
| core + artifacts (Act note) | 3,612 |
| full (all five) | 4,919 |

## Live check

Installed binary `560d2c95…`, real DeepSeek, TUI driven through tmux.
Workspace `/tmp/tui-delta-test`, session `ses_tui_delta_20261122`, journal
`f6a7c4e3-e5dc-4f01-98d3-a84bfb0986c1`.

Task: `Execute: create /tmp/tui-delta-test/note.txt with exactly the text: delta`

Result: `completed`, acceptance receipt `accepted` (digest `be0bc667…`), file
5 bytes, sha256 `4f4a9410…`. 6 model calls / 5 tool calls, 18.7s wall
(14.2s in the five measured model steps).

| Metric | Before (`0dcb6885`) | After (`f6a7c4e3`) |
|---|---|---|
| Advertised tools | 16 | 16 |
| System tokens (step 0) | 5,168 | **3,862** |
| Tool schema tokens | 3,099 | 3,099 |
| Fixed system + schema | 8,267 | **6,961** |
| Context tokens | 8,487 | **7,182** |
| Input tokens | 8,729 | **7,626** |

The live system message contains `Delivery efficiency for artifact/file tasks`
and contains none of `For a page or UI deliverable` (page),
`Before a non-trivial build` (capability), `The request may be a follow-up`
(followup), or `Conclusion contract for DirectAnswer` (answer style). The
first system message is 3,688 tokens by `estimate_tokens`; the remaining
system messages bring the logged step-0 total to 3,862.

## Tests

- `prompt_deltas_are_added_only_when_requested`: core omits every delta, the
  full prompt contains all of them.
- `prompt_budgets`: core < 3,000, answer < 3,200, Act note < 4,000 tokens.
- `act_prompt_sends_only_the_deltas_its_tools_can_use`: a core-tool Act note
  request omits page/capability/followup/answer-style; a page Act request
  includes the page block.
- Gates: lib 1423/0/4, bin suite exit 0, fmt, strict clippy, eval 8/8.

## Limits

- One live run; model output is nondeterministic.
- No live page build in this slice. Page selection is covered by the
  deterministic runtime test; the operational page checklist is still
  injected separately for page Act runs.
- The CLI `run` still advertises the full tool surface, so it also sends the
  capability delta; only the TUI core surface drops it.
- The system prompt is now below the 4k target for an Act note (3,862).
  Fixed system + schema is 6,961, not 4k, because the core tools still cost
  3,099 tokens of schema.
