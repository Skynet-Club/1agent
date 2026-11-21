# TUI continuation live checks (2026-11-21)

Goal: verify the installed TUI really continues the same conversation when
reopened with `--session <id>`, that a continuation skips capability
preflight, that a new session still runs it, and what a repeated task does
before the already-done gate exists.

Environment: installed binary sha256
`4eb4d924c42aab5aca643983e2d0ba5df93a395c599fdce7da9bc6e0e381c22b`,
real DeepSeek, TUI driven through tmux. Workspace `/tmp/tui-cont-test`,
journal dir `/tmp/tui-cont-journal`.

## Note-file session — same conversation, tiny edit

Session `ses_tui_cont_a_20261121` (file
`~/.1a/sessions/4fd1adb1ffa000d7/ses_tui_cont_a_20261121.jsonl`).

| Turn | Task | Session lines | Models | Tools | Wall | Result |
|---|---|---|---|---|---|---|
| A1 | create `note.txt` with `first` | 2 | 7 | 6 | ~16s | `first` |
| A2 | change `note.txt` to `second` | 4 | 7 | 6 | 23.5s | `second`, sha `16367aac…` |

A2’s model request contains the A1 turn. The session file grew from 2 to 4
lines; a new file was not created.

Note: a plain file write does **not** trigger `capability_preflight`, so this
session cannot prove the preflight skip. It proves conversation continuity.

## Page session — preflight skip and repeat baseline

Session `ses_tui_cont_p_20261121` (file
`~/.1a/sessions/4fd1adb1ffa000d7/ses_tui_cont_p_20261121.jsonl`).

| Turn | Task | Session lines | Preflight | Models | Tools | Wall | Result |
|---|---|---|---|---|---|---|---|
| P1 | new session: build page, h1 `Alpha` | 2 | **1** | 9 | 8 | 43.8s | `Alpha`, sha `2ff56cd0…`, `write_file` |
| P2 | same session: change h1 to `Beta` | 4 | **0** | 9 | 8 | 31.5s | `Beta`, sha `d445eb7d…`, `edit_file`, `verify_artifact` |
| P3 | same session: repeat P1’s full build task | 6 | **0** | 7 | 5 | 35.4s | rebuilt page back to `Alpha`, sha `2b1b6b18…`, `write_file` |

P1 is the new-session control: it compiled a capability contract
(`preflight=1`). P2 is the continuation: no `capability_preflight_resolved`
event, and the prior turn is in the model request. P2 edited the existing file
in place instead of rebuilding it.

P3 is the current repeat-task baseline: with session context the repeat is
cheaper than the original 58-step run, but the model still acts — it rewrote
the page (`write_file`) instead of asking “already done; verify or new
version?”. The already-done gate is the next fix.

## Negative control

Session `ses_tui_cont_b_20261121` created a new conversation file and
`note_b.txt`; its preflight count is 0 only because a plain file write has no
capability requirement. Use P1, not this run, as the new-session preflight
control.

## Limits

- One run per arm; model output is nondeterministic.
- The TUI was driven by tmux `send-keys`; capture-pane is blank in the
  alternate screen, so verification reads journals and session files.
- The already-done gate is not implemented; P3 is the baseline it must beat.
