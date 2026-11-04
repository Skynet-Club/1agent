# ADR 0014: Token Headroom, Session Workspace and Local Presets

Status: accepted

Date: 2026-11-03

## Context

Long TUI sessions can repeatedly inject old tool output and recent turn summaries,
increasing provider cost. Users also need to switch sessions, inspect or change the
workspace and export local evidence without memorizing CLI options. Token accounting
must remain honest: character budgets are not exact provider token counts.

## Decision

Expose and persist five resource limits:

- maximum reasoning steps;
- maximum tool calls;
- maximum provider attempts;
- maximum context characters;
- run timeout.

`/config` edits these bounded values in the active session. `/config reset` returns
to the original CLI defaults. The values apply to subsequent TaskRequests.

Token and context controls have two layers:

1. Runtime context compaction clears old, non-recent tool result content when the
   character budget is exceeded.
2. `/context compact` creates a deterministic local summary of up to eight recent
   completed runs. Subsequent turns inject that summary instead of the normal recent
   six-turn facts until a new journal invalidates it.

`/context show` reports the configured context-character budget, currently injected
fact characters and remaining character headroom. These are character measurements,
not billing-token claims.

Session metadata version 2 adds optional:

- preferences;
- compacted context summary;
- preferred workspace;
- mentioned paths;
- archived state.

Version 1 and earlier version 2 files remain readable through serde defaults.

`/session` supports show, list, new, resume, fork, rename, archive and recoverable
delete. Deletion requires the exact ID twice and moves the session under `.trash`.
Forks retain journals, workspace, mentions and preferences but start without the old
mission/plan binding, avoiding cross-session plan-path reuse.

`/workspace` supports show, canonical `cd`, bounded mention paths, git diff, safe
`AGENTS.md` initialization and git worktree list/add. New paths must remain under
configured read roots. `AGENTS.md` creation uses create-new and cannot overwrite.

`/preset` stores named, bounded task instructions outside the model context. `/export`
writes session/report/evidence Markdown to an explicit new file. `/copy` copies the
latest answer through the existing native/OSC 52 clipboard path.

## Consequences

- Users can reduce context and attempt budgets without CLI restarts.
- Context compaction is local and provider-free.
- Character headroom is visible but not misrepresented as exact token usage.
- Session switching does not silently reuse incompatible Master plan paths.
- Workspace changes are persisted but constrained by read roots.
- Presets add no provider calls until explicitly run.
- Local exports and deletes are bounded, explicit and recoverable where practical.

## Verification

- session metadata round-trips preferences, workspace, mentions, context summary and
  archive state;
- fork starts without the source mission plan;
- archive and recoverable trash are tested;
- invalid resource budgets and workspaces fail closed;
- context recap/compaction and character headroom are tested;
- config set/reset updates future requests;
- presets round-trip and move to trash;
- `AGENTS.md` cannot be overwritten and worktree path conflicts fail before git;
- CLI line-mode E2E exercises status, workspace, context, config, preset, export,
  copy, rename, fork and session listing;
- real PTY validation covers the interactive path.
