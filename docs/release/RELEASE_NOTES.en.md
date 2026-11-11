# Release Notes — 0.96.1 TUI, Context and Prompt Library

**English** · [简体中文](https://github.com/Skynet-Club/1agent/blob/main/docs/release/RELEASE_NOTES.md)

Date: 2026-11-11  
Status: **R1 public pre-release (patch), not production-stable**

GitHub pre-release: <https://github.com/Skynet-Club/1agent/releases/tag/v0.96.1>

## Scope

0.96.1 is a patch release on 0.96.0 focused on TUI readability, context
budgeting, the prompt library and answer formatting. It does **not** include the
0.97 Reliable Loop scope: streaming, parallel read-only tools, incremental
journal or crash recovery.

## Highlights

### 1. Answer readability

- The conclusion fence now renders Markdown hierarchy: `##` subheadings,
  ordered and unordered lists, `- [x]` task boxes, `**bold**` key labels,
  `---` dividers, link-colored URLs/local paths and column-aligned tables.
- The system prompt asks for one result sentence first, then short paragraphs,
  bullets or a comparison table for multi-dimensional answers; simple answers
  stay one short paragraph.
- A live `deepseek-flash` canary returned one summary sentence, a capability
  table, three bullet points and a defaulted question instead of one wall of
  text.
- URLs glued to Chinese text (for example `参考：https://...`) are link-colored.

### 2. Prompt library

- `/` still searches the 48 curated Chinese cards and shows three featured
  prompts on an empty query.
- `//` opens the complete source library: the 56 verbatim Feishu rows are
  grouped into 32 author/topic entries and labeled by category. Snapshot and
  importer:
  - `docs/evidence/prompt-sources-2026-11-11.ndjson`
  - `scripts/import-prompt-sources.py`
- Insertion is the complete body only: the full Chinese translation in Chinese
  mode or the full English original in English mode. Nothing is truncated or
  rewritten, and no author/topic header or reference line is written into the
  composer; those stay in the drawer row.
- On wide terminals a side pane previews the complete body, author, category,
  fragment count and character count; `Enter` inserts.
- Inserting the same body twice is skipped.
- New `/config set language zh|en`: the source library works in both languages;
  the curated English `/` subset lists only the 14 cards with verbatim English
  source text, and cards without an English source stay hidden instead of
  silently falling back to Chinese.
- Completeness gate: `tests/prompt_sources.rs` reparses the 56-row snapshot and
  compares every group's joined `original`/`translation` and row count; import
  truncation or rewriting fails the build.

### 3. Context and budgets

- Context budgets are token-based: 1,000,000 tokens by default, automatic
  compaction at 786,432; `--max-context-tokens` replaces `--max-context-chars`.
- Legacy character budgets migrate on read: the old 100,000-character default
  maps to 1M tokens; custom values use `ceil(chars / 3)`; old session/task JSON
  remains readable.
- TUI, `/status` and `/context` report tokens; provider-reported
  `usage.input_tokens` is authoritative after each response.
- Interactive TUI runs no longer stop at six steps: steps, tool calls, provider
  attempts and run duration default to unlimited (`0`); the status line and
  `/config` show `不限`. Headless `run`, the daemon and subagents keep finite
  budgets; interactive TUI provider calls keep a 300-second transport ceiling.
- Hidden caps removed: the runtime provider-attempt clamp (was 16) and the TUI
  step/tool validation caps (was 64).

### 4. TUI visual system and interaction

- Dark-green backdrop `#06100A`; input panel `#08140D`, border `#3F644B`, panel
  text `#487748`, white input text.
- The input-panel top border carries state/step and token/cache/context metrics;
  narrow terminals fold metrics inside the fence. The bottom border carries run
  status, model and mode. The context meter uses `─` cells (20 wide, 10 narrow).
- Welcome wordmark: leading `1` bright green, `Agent`/caption deep green,
  `SKYNET.CLUB` bright green.
- All popups (`/details`, `/help`, `/` drawer, `Ctrl+T`) share green surfaces
  `#0A1F12`, selected rows `#143D24`, borders `#3F644B` and bright-green titles.
- Work log: action names, `•` bullets and links are bright green; details and
  `⎿` results stay deep green `#487748` and unbolded. `第 N 步` counts actual
  model requests; `Ctrl+O` opens run details; Bash results include a bounded
  output preview.
- Running state is a two-line activity card driven by the run journal with
  bilingual stages, model step and a minimal fence. A persistent Master-Agent
  status card covers mission understanding, planning, approval, assignment,
  starting, worker running, collecting, verification and reporting, with a
  next-command footer.
- User messages use the input-panel background and white text; right-click
  paste is scoped to the composer.
- Composer mouse selection works across the whole composer. Editing is hardened
  against stale/mid-grapheme cursors, with a 64 KiB input cap, a deterministic
  editing fuzz test and a PTY paste/backspace/space fuzz script. TUI panics
  restore the terminal and append to `$HOME/.1a/tui-crash.log`; transient
  input-read errors are retried.
- The final answer fence `╭─ 结论 Conclusion ... ╮` has dimmed deep-green
  borders, a bright-green bold title and a white body. Structured
  claim/evidence/assumption/constraint/unknown/conflict/flip sections use deep
  green `#487748`; only the conclusion body stays white.
- The status/work marker is the smaller `•`; normal activity uses one cyan
  accent, while green/amber/red are reserved for completed, waiting/retrying and
  blocked outcomes.

## Verification

- `release-check.sh`: fmt, `clippy -D warnings`, full tests, HTTP e2e, ci and
  TUI interaction, including 400 mixed edit events.
- `verify-claims.sh`: claims table well-formed; see the current run output for
  exact verified/partial/no-evidence counts.
- Real PTY evidence lives in `docs/evidence/tui-2026-11-10/`; answer formatting
  was also checked with a live provider canary.

## Known limits

- Linux x86_64 only; Windows builds are not published.
- 0.96.1 is still an R1 public pre-release, not production-stable; interfaces,
  configuration and data formats may change.
- External side effects (commit, push, publish, spend) remain behind
  authorization/Policy gates and are never reported as silently executed.
- The source library groups fragments by author/topic; it is not a full-article
  reader. Use the original Feishu links for single-row context.
- The curated English `/` layer has 14 cards; `格式锁定` (Riley Goodside) and
  `写成脚本` (Karpathy) still lack an English original, so they stay hidden in
  `/` English mode.

## Assets

- `1agent-0.96.1-linux-x86_64.tar.gz` — Linux x86_64 package;
- `SHA256SUMS` — archive and Linux provenance checksums;
- `linux-x86_64.json` — Linux build provenance.
