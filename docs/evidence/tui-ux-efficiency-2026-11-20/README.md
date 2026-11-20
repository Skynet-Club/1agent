# 1Agent TUI UX / efficiency fixes (2026-11-20)

Status: **landed; long transcript history, consistent code fences, neutral
run-time notices, and an efficiency prompt/nudge**

## Fixed

1. **Scrollback**: the transcript kept only the last 200 entries and dropped the
   oldest (`remove(0)`), so a 59-step run only reached about step 26. The cap is
   now 2,000 entries. A test pushes 300 entries and asserts the first one still
   exists.
2. **Code blocks**:
   - line numbers now appear for every non-diff block, not only blocks longer
     than 3 lines;
   - the closing `╰─…╯` fence is covered by a test;
   - fenced code inside a structured item (claim/evidence/detail) is routed
     through the markdown renderer instead of being flattened into bold prose;
   - code content and syntax colors render without `BOLD`; the fence border may
     stay bold.
3. **Mid-run append / commands**:
   - the composer already queues Enter while a run is active; queued messages
     run after the current run and can be recalled with Up;
   - state-changing slash commands and session picker new/switch actions now say
     the command is available after the current task and suggest Enter to queue,
     instead of the error-like “无法在运行中执行”;
   - no state-changing command is executed mid-run (still fail-closed).
4. **Efficiency**:
   - the agent system prompt gains rule 11: batch related shell work, do not
     repeat an identical read/audit, try a missing skill at most twice, write
     once, verify once, then answer;
   - a transient system nudge is injected at step 30 and step 40 (“finish the
     deliverable and verify once”); it is not appended to the durable
     conversation and there is no hard step cap (TUI `max_steps` remains 0 =
     unlimited).

## Tests / gates

Focused tests (`focused-tests.txt`):

- `transcript_history_keeps_more_than_the_old_200_entries`
- `short_code_blocks_show_line_numbers_and_the_closing_fence`
- `code_fences_inside_structured_items_still_render_as_code`
- `code_fence_uses_the_dark_military_green_palette` (now also asserts code
  content is not bold)
- `system_prompt_carries_delivery_efficiency`
- `efficiency_nudges_fire_at_the_checkpoint_steps`

Full gate (`gate.txt`):

- lib 1,366 passed / 0 failed / 4 ignored;
- bin 68 passed;
- eval `evals/core.json` 8/8 PASS;
- CLI e2e 39 passed with the one pre-existing session-limit failure skipped;
- strict clippy and fmt clean.

## What this does not prove

- The exact “中途追加” error text was not supplied. The visible command/session
  messages were changed to neutral, actionable notices, but if the error came
  from another path (for example a provider/session failure), the original text
  is still needed.
- The orphan journal `ff8c3136…` has a `RunStarted` with no terminal event; the
  audit flags `journal has no run stop event` (`orphan-audit.txt`). Automatic
  startup recovery/reconciliation for such journals is not implemented.
- Code blocks inside **tool results** (not model answers) still render as work
  results, not as `Code` entries; the fix targets assistant and structured-item
  code.
- No live visual TUI check: `agent_exec` is blocked by the skynet-term 128
  session limit, so the evidence is TestBackend unit tests plus the full gate.
- The prompt/nudge should reduce step counts, but no live 59-step rerun was done
  in this slice; measure it on the next real session.

## Release provenance

Feature commit `09729be`; release binary `e613bf2b…` installed as
`~/.local/bin/1agent`; previous MA-014 build `5f93edb7…` backed up as
`1agent-pre-tui-ux-2026-11-20`. Installed smoke: `1agent --version` and
`1agent run --help` exit 0. Details in `release.txt`; source and evidence
digests in `sha256.txt`.

## Blocker

`agent_exec` still returns
`create_failed: session_limit：活动会话最多 128 个`; final verification used the
visible Bash tool and its log is captured here.
