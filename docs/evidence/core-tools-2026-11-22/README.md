# Core tool surface live check (2026-11-22)

Goal: reduce the TUI's advertised tool surface and the fixed per-call token
overhead without removing access to the rest of the registry.

## Change

- `CORE_TOOL_NAMES` (14): read/write/edit/search files, read_document,
  terminal_create/exec, preview_file, browser, acceptance_gate,
  verify_artifact, clarify, skill_search, skill_list.
- When `core_tools` is on, both the model request and the journal capability
  snapshot are filtered to this set; `describe_self` and `request_tools` stay,
  and capability-contract actions that are explicitly selected remain.
- Other registered tools stay available through `request_tools`; a test
  requests `web_search` back into the next model request.
- The TUI enables `core_tools`; the CLI `run` keeps the full surface for now.

## Measurement

Task: `Execute: create /tmp/tui-core-test/note.txt with exactly: core`
(installed binary, real DeepSeek, journal `0dcb6885`). The file was written as
`core`; the run used 5 model calls / 4 tools.

| Metric | Before (`4f6cc777`) | After (`0dcb6885`) |
|---|---|---|
| Advertised tools | 38 | **16** |
| System tokens (first call) | 5,880 | 5,168 |
| Tool schema tokens (first call) | 6,034 | **3,099** |
| Fixed system+schema | 11,914 | **8,267** |
| First context tokens | 12,287 | **8,487** |

The advertised set is: acceptance_gate, browser, clarify, describe_self,
edit_file, preview_file, read_document, read_file, request_tools,
search_files, skill_list, skill_search, terminal_create, terminal_exec,
verify_artifact, write_file.

Caveat: the “before” run was a page task, so the system-prompt/task-profile
part is not a controlled A/B. The tool-schema drop is direct: 38 → 16 tools,
6,034 → 3,099 tokens.

## Tests

- `core_tools_reduce_the_advertised_surface`: advertised set includes
  read_file/browser, excludes web_search/spawn_agent, at most 18 tools.
- `core_tools_can_be_requested_back`: a `request_tools` call for web_search
  makes it appear in the next model request.

## Limits

- 16 tools, not the 6–8 target; the system prompt (5.2k) is now the larger
  fixed cost. Instruction deltas are the next P2 slice.
- Excluded tools need an explicit `request_tools` call; a model that does not
  ask will not see them.
- CLI `run` still advertises everything; only the TUI uses the core set.
