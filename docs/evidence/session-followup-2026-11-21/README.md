# Session follow-up live check (2026-11-21)

Goal: prove that a second turn in the same session really continues the
conversation, skips capability preflight, and edits the existing artifact
instead of rebuilding it. All runs use real DeepSeek through
`~/.1a/config.json`; no mock provider.

Installed binary sha256
`4884ba1db58c8a0f80d6a152c7b2f2dcbacaa40e9168a4108d75cd0cbea2b1fc`
(`1.0.0-rc.13`). Backup `1agent-pre-continuation-2026-11-21`
(`2938bef0…`).

## Three-turn CLI run (debug binary from the same commit)

Workspace `/tmp/followup-live-2026-11-21`, session `followup-live`.

| Turn | Objective | Mode | Preflight | Models | Tools | Wall | Result |
|---|---|---|---|---|---|---|---|
| 1 | build a single-file HTML at `index.html`, h1 `Test` | act | **1** | 10 | write_file, browser×2, preview, acceptance×3 | 46s | completed, `c60e48aa…` |
| 2 | change h1 to `Test 2`, keep everything else | act | **0** | 12 | read, edit, preview, browser×3, acceptance×3 | 53s | completed, `a2dccec3…`, `<h1>Test 2</h1>` |
| 3 | “what is the h1 now? answer only the title” | answer | **0** | 1 | none | 4.3s | completed, “Test 2” |

The turn-2 model request contains the turn-1 user objective and the turn-1
assistant conclusion; turn 3 contains both prior turns. Turn 2 has no
`capability_preflight_resolved` event, and turn 3 has no tool calls and does
not touch the file.

## Installed binary, two-turn repeat

Workspace `/tmp/followup-installed-2026-11-21`, session `installed-followup`.

| Turn | Objective | Mode | Preflight | Models | Tools | Wall | Result |
|---|---|---|---|---|---|---|---|
| 1 | build a single-file HTML, h1 `Alpha` | act | **1** | 7 | write_file, browser×2, preview, acceptance | 29s | completed, `7ceb258e…` |
| 2 | change h1 to `Beta`, keep everything else | act | **0** | 8 | read, edit, preview, browser×2, acceptance | 22s | conditional (accepted), `357c7d44…`, `<h1>Beta</h1>` |

Turn 2 again has no capability preflight and the prior turn in the request.
The h1 edit changed the file from 1337 to 1336 bytes; preview and three-viewport
audit still ran.

## What this verifies

- A session with prior turns is treated as a continuation: **no capability
  preflight**, no new contract compilation.
- The artifact is edited in place (`edit_file`), not rebuilt or replaced by a
  new page; the old file sha changes only by the intended edit.
- A pure question still lands in `Answer`, makes one model call, uses no tools,
  and leaves the file untouched.
- The per-run journal still records every event; the session JSONL is the
  conversation projection.

## Limits

- The TUI was not driven live in this check; its runtime path is the same
  `with_session_path` call and it is compiled, but a TUI acceptance run is still
  pending.
- Capability preflight is skipped for **any** session with prior turns; a new
  task should start a new session.
- Steering, tool/reasoning parts and compaction entries are not implemented yet.
- The debug run produced an accidental duplicate question turn from a repeated
  invocation; the duplicate was removed from the test session and its journal
  is not included here.

## Files

`journals/` contains the five journals above (debug build/modify/question,
installed build/modify). `installed-index.html` is the final installed-page
artifact (`357c7d44…`).
