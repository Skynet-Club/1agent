# MA-013 TUI journal cursor (2026-11-20)

Status: **landed; the squad preview reads a bounded tail once, then only new
events**

## Scope

The squad panel's child-journal preview used a fresh `JournalCursor` on every
refresh and read the whole journal from byte 0. `refresh_squad_panel` also
invalidated the preview text every refresh, so the read repeated even when no
new events existed. The fix keeps a per-subagent cursor and timeline in
`TuiState`.

## Landed

- `journal::read_events_tail(path, max_bytes)` reads at most the last
  `max_bytes`, skips the first partial line, parses complete events, and returns
  a `JournalCursor` at the end of what it parsed. A later
  `read_events_incremental` returns only appended events.
- `SubagentPreviewCache` in `tui.rs` holds the journal path, `JournalCursor` and
  the last 80 rendered timeline lines. `TuiState::subagent_preview_cache`
  creates it on demand; `session_picker_set_squad` prunes caches for records no
  longer in the squad.
- `subagent_preview`:
  - first read for a journal path: `read_events_tail` with a 256 KiB cap;
  - later reads: `read_events_incremental` only;
  - on a read error (including a shrunk journal): reset and retry the bounded
    tail once, otherwise show "journal is temporarily unreadable";
  - renders the last 18 lines from the persisted 80-line timeline.
- The preview function now takes `&mut TuiState`; `sync_session_picker_preview`
  passes it and uses the selected record id.

## Tests

- `tail_read_is_bounded_and_then_continues_incrementally`: a 512 KiB event is
  skipped by a 64 KiB tail read; only the last event is returned and the cursor
  lands at EOF; appending one event returns exactly that event.
- `subagent_preview_reads_the_child_journal`: existing preview content and
  lineage still render.
- `subagent_preview_appends_only_new_events`: first preview reads the journal;
  after appending a tool call, the second preview contains the new line, keeps
  the earlier timeline, and leaves the cursor at the new EOF.

Full gates (`gate.txt`, `eval.txt`):

- lib 1,358 passed / 0 failed / 4 ignored;
- bin 68 passed;
- CLI e2e 39 passed with the one pre-existing baseline failure skipped
  (`cli_tui_agents_refresh_uses_configured_profiles`, also fails on unmodified
  HEAD `264fa34`);
- MAST fault injection 7 passed;
- eval `evals/core.json` all scenarios PASS;
- strict `cargo clippy --offline --all-targets -- -D warnings` clean;
- `cargo fmt --check` clean.

## What this does not prove

- The first read still parses up to 256 KiB of the newest events; a single
  enormous event can consume that budget and hide older timeline lines.
- The cache is per record id and bounded to 80 rendered lines; switching away
  and back reuses the cursor, but a changed journal path resets it.
- No wall-clock assertion: the guarantee is a bounded byte read plus
  incremental appends, not a measured frame budget.
- The TUI still refreshes on its normal cadence; this change removes the
  repeated full journal parse, not the refresh itself.

## Release provenance

Feature commit `7d83da7`; release binary `9277e48f…` installed as
`~/.local/bin/1agent`; previous MA-012 build `70033348…` backed up as
`1agent-pre-tui-journal-cursor-2026-11-20`. Installed smoke: `1agent --version`
and `1agent run --help` exit 0. Details in `release.txt`; source and evidence
digests in `sha256.txt`.
