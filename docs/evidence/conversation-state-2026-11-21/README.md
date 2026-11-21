# Conversation state — work ledger + Answer/Ask/Act (2026-11-21)

Status: **P0 landed and live-verified**. Binary version remains
`1.0.0-rc.13`; installed binary sha256
`bdce2d77db36504db2edc626c07382d9ddf2ac707a9f8642d895048c87d75959`.

## What changed

1. **Work ledger** (`src/work_ledger.rs`)
   - Append-only JSONL at `~/.1a/work-ledger/<workspace-hash>.jsonl`
     (`ONEAGENT_WORK_LEDGER_DIR` overrides the root for tests).
   - Entry: run id, session id, time, objective, interaction mode, status,
     artifacts (`path`, `sha256`, `bytes`, `fresh`), reused paths, blockers.
   - Loaded at run start (last 5 entries) and injected as `WORK LEDGER` with a
     live hash check of referenced artifacts. Facts come from tool
     observations, not model claims.
2. **Interaction mode gate** (`src/interaction_mode.rs`)
   - `Answer` / `Ask` / `Act`, decided before capability compilation.
   - Question + prior-work reference (`昨晚/上次/还是/这版`) → Answer.
   - Explicit build/change/version wording → Act.
   - Vague change (`优化一下`) → Ask.
   - Journaled as `InteractionModeSelected`.
3. **Hard mask**
   - Answer mode skips capability preflight, page checklist and the design
     completion gate; write/`acceptance_gate`/`preview_file`/`verify_artifact`/
     `browser`/`resolve_capability` tools are removed from the advertised and
     executable sets; `request_tools` cannot re-advertise them.
   - Answer tool budget ≤2, steps ≤4; Ask steps ≤2.
4. **Truth preservation**
   - The unrepairable-answer fallback reports actual tool counts and files,
     and never says “本轮没有执行任何外部动作” when tools ran.

Plan and research basis: `docs/1.2-conversation-state-plan.md`.

## Gates

- `cargo fmt --check`: clean.
- `cargo clippy --offline --all-targets -- -D warnings`: clean.
- `cargo test --lib`: **1404 passed / 0 failed / 4 ignored**.
- Full `cargo test`: 39 passed / 1 failed — the known
  `cli_tui_agents_refresh_uses_configured_profiles` skynet-term 128-session
  environment failure.
- `eval --suite evals/core.json`: **8/8 PASS**.

## Live runs (final binary)

### 1. Build run — Act + ledger write

- Journal: `journals/240198fc-831b-406c-88bf-9eca1e0ca25c.jsonl`
- 60.5s; `mode=act`; status `completed`.
- Wrote `/home/skynet/work/conversation-state-test/index.html`, 8,872 bytes,
  sha256 `144049de4ba8…`; ledger entry records `fresh=true`.
- JSON: `build-final2.json`.

### 2. Follow-up question after the build — Answer, no delivery flow

- Journal: `journals/849e92bb-c44e-435b-bb15-c0acb4ac168a.jsonl`
- 14.7s; `mode=answer`; 2 read-only `describe_self` calls; **0 capability
  preflights, 0 acceptance gates**; status `completed`.
- Answer traces the file through 7,475 → 9,958 → 8,872 bytes, identifies the
  latest write, and asks for last night's file/hash to diff. It does not
  re-audit or rebuild.
- JSON: `question-final2.json`.

### 3. Seeded “last night” workspace — the user's exact failure

- Workspace: `/home/skynet/work/conversation-state-old`, old `index.html`
  57 bytes, sha256 `a595bbde…`.
- Ledger entry seeded with yesterday's timestamp and the old hash.
- Journal: `journals/c8069124-bb6a-4d26-9304-d9d76dc3016c.jsonl`
- **16.8s; `mode=answer`; 2 read-only tools; 0 capability preflights;
  0 acceptance gates; `completed`.**
- Answer: “不是重新设计，也不是拿昨晚的成品糊弄你——工作区里那份
  index.html 根本不是页面，是昨晚留下的 57 字节占位……本次会话我没有
  写过任何文件。”
- Old file unchanged after the run (same sha256, same size/mtime).
- JSON: `question-old-final2.json`.

### 4. Real `join-us` workspace without a ledger

- Journal: `journals/011cb5e7-0f4b-4127-8d0f-5191f28b27aa.jsonl`
- 24.1s; `mode=answer`; 2 read-only tools; **0 capability preflights,
  0 acceptance gates**; status `conditional`.
- It says it has no delivery ledger for this workspace and asks for the link or
  sha256 instead of running an audit. This is the honest behavior when the
  feature predates the workspace history.
- JSON: `question-join-us.json`.

## Honest limits

- Existing journals are not backfilled into the ledger. A workspace whose
  history predates this build answers “no ledger for this workspace” until a
  new run writes an entry. A `ledger rebuild` backfill is P1.
- The mode gate is a heuristic. A false Answer costs one retry; a false Act is
  avoided by defaulting non-action text to Act only when it is not a question
  or prior-work reference. Journaled decisions allow replay evaluation.
- Answer mode still permits up to two read-only tools (`read_file`,
  `search_files`, `session_search`, `memory_recall`, `describe_self`). The
  final seeded run used two `read_file` calls; the build follow-up used two
  `describe_self` calls.
- The ledger is keyed by the first `read_root`. The TUI's default workspace is
  `/home/skynet`, so project subdirectories share one ledger until project
  binding lands in P1.
- `Ask` currently covers vague changes and short follow-ups; it is not a full
  clarification planner.
- The running TUI holds the old inode until restarted.
