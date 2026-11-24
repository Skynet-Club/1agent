# TUI `/fast` toggle — 2026-11-24

Adds an in-session fast/full policy command so the TUI no longer depends only
on `ONEAGENT_AUTO_FAST=0`.

## Behavior

- `/fast` or `/fast show`: show the current session policy.
- `/fast on` or `/fast auto`: low-risk automatic fast (the default). Only a new
  Act task with a Fast profile, write permission, no injected facts, no
  continuation and no page deliverable takes the four-tool fast path; pages,
  continuations, injected-context tasks, verify/new-version requests and
  high-risk runs stay on Core 7/full.
- `/fast off`: every task uses Core 7/full.
- The command is Core-scope, inline, lives in the “模型与权限” drawer group, and
  is unavailable while a run is active.
- The policy is stored in `SessionPreferences.fast_policy` and applied before
  every TUI task. Reopening the same session (for example with `--continue`)
  restores it; a new session without a preference uses `ONEAGENT_AUTO_FAST`.
  A resume still reconstructs the checkpoint's stored `fast_mode`, not the
  current toggle.
- The runtime exposes `FastModePolicy::{Off, Auto}` and
  `AgentRuntime::set_fast_policy`; the CLI `--fast` / `--no-fast` semantics are
  unchanged.

## Tests and gates

- `tui_commands::tests::fast_is_a_registered_core_command` — registry, scope,
  grouping, inline behavior, usage string.
- `runtime::tests::fast_policy_maps_to_the_runtime_fields` — Off/Auto mapping.
- `tui_rendering_tests::fast_command_persists_in_session_preferences` —
  `/fast off` changes the state and the session file; reopening the session
  still shows `Off`; `/fast on` restores `Auto`.
- The session round-trip test asserts `fast_policy: off` survives save/reopen.
- Command-count and drawer-order tests updated for the new Core command.
- fmt, strict clippy, lib **1452 passed / 0 failed / 4 ignored**, bin **72 passed
  / 0 failed**; core eval 8/8 unchanged.

## Live TUI checks

Driven through tmux with isolated `ONEAGENT_HOME` and session directories. Each
run used a fresh TUI session, so the first task was not a continuation.

`/fast off`:

- `/fast show` displayed `off（全部走 Core 7/full）`.
- Task wrote `workspace-off/out.txt`; answer completed in 12.5 s with
  `write_file` + two `acceptance_gate` calls (the second `accepted`), no
  terminal.
- Journal `FastModeSelected {enabled: false, reason: "fast mode off"}`;
  tool surface was the 7-tool Core set including `acceptance_gate`.
- No delivery was queued. Transcript: `fast-off.txt`.

`/fast on`:

- `/fast show` displayed `auto（低风险自动 fast）`.
- Task wrote `workspace-on/out.txt`; **2 steps / 1 tool / 2.2 s**.
- TUI notices: `交付已入队：run 2e4db38d…` then
  `交付验收：新处理 1 条（accepted 1 / conditional 0 / rejected 0），失败 0 条`.
- Journal `FastModeSelected {enabled: true, reason: "auto: small new act
  delivery"}`; tool surface exactly
  `[read_file, terminal_create, terminal_exec, write_file]`; snapshot root
  `workspace-on`.
- Receipt `accepted` with 0 failed checks: `receipt-on.json`.
- Transcript: `fast-on.txt`.

## Persistence live check

Same workspace and journal directory throughout; the TUI session is
`ses_a157acb1f8504f7da4e5220d7461bfd5`.

1. First launch: `/fast off` displayed `已保存到会话偏好`.
   `runs-persist/.sessions/ses_a157…/session.json` contains
   `"fast_policy": "off"` (copy: `session-off.json`; transcript:
   `persist-off.txt`).
2. Relaunch with `--session ses_a157…`: `/fast show` displayed
   `off（全部走 Core 7/full）` **without** typing `/fast off`
   (`persist-restore-off.txt`).
3. `/fast on` wrote `"fast_policy": "auto"` (`session-auto.json`). Relaunch
   with the same session: `/fast show` displayed
   `auto（低风险自动 fast）` (`persist-restore-auto.txt`).

## Binary

Final installed `~/.local/bin/1agent` sha256
`eb9e6d54087324f5dd35c8a7…`; `/fast`-without-persistence `eb878761…` is saved
as `~/.local/bin/1agent-pre-fast-persist-2026-11-24`; the earlier
`1da41f5d…` is saved as `~/.local/bin/1agent-pre-fast-toggle-2026-11-24`.
