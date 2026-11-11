# Interactive TUI

Date: 2026-11-02

Status: interactive TTY interface implemented; piped hosts use a line-oriented fallback

## Start

```bash
cargo run --offline -- tui
```

With a provider configuration:

```bash
cargo run --offline -- tui --config configs/example.json
```

Provider turns default to the configured `--max-steps` value so that multi-step
tool use can still produce a final answer. Override them without changing the
reasoning-step budget:

```bash
cargo run --offline -- tui --max-provider-attempts 10
```

The effective provider-turn budget is clamped to the runtime safety range
`1..=16`.

## Host binding and command scope

1A has two command surfaces:

- **Core**: available in standalone `1agent tui` and in Workshop;
- **Master**: available only when Workshop starts 1A as `main-coordinator`.

The current compatibility binding recognizes:

```text
SKYTERM_EXPERT_INTERACTIVE=1
SKYTERM_AGENT_ID=main-coordinator
SKYTERM_AGENT_RUNTIME=1agent
SKYTERM_SESSION_ID=term-...
```

The versioned host contract adds:

```text
SKYTERM_HOST_CONTEXT=skynet-workshop
SKYTERM_AGENT_ROLE=master
SKYTERM_HOST_CONTEXT_VERSION=1
```

Conflicting or incomplete host signals fail closed to standalone mode. The command drawer
uses the detected host and lifecycle stage to decide which commands are visible. Typing a
Master command in standalone or Worker mode is rejected even though the command may be
registered internally.

Master commands that are registered but not implemented yet are shown with `（开发中）` only
inside the Workshop Master console. Standalone mode never shows them.

## Current command inventory

Standalone 1A and Workshop Worker expose 24 Core commands:

```text
/help /status /details /journal /clear /quit
/frame /challenge /evidence /audit /decide /report /postmortem
/model /permissions
/session /workspace /context /preset /tools /export /copy /cost /config
```

Workshop Master additionally exposes 15 production-control commands:

```text
/mission /triage /plan /approve /agents /assign /dispatch
/board /watch /attention /respond /control /artifacts /verify /timeline
```

The Master drawer is stage-aware: no mission shows 26 commands, framing 28, approval 29 and
running 34. Worker mode never exposes Master commands.

## First-principles commands

The Core command surface includes:

- `/frame [context]`: explicitly runs evidence research to separate facts, assumptions,
  constraints, unknowns, criteria and flip conditions;
- `/challenge [context]`: explicitly runs diagnosis to red-team a plan or conclusion;
- `/decide [context]`: explicitly runs decision analysis over alternatives and tradeoffs;
- `/postmortem [context]`: explicitly runs diagnosis for root cause, prevention and regression;
- `/evidence`: renders the latest answer's claims, sources, unknowns, conflicts and flip
  conditions from the run journal;
- `/audit [run|all]`: runs deterministic journal audit over the latest run or all session runs;
- `/report`: renders a bounded task report from the latest journal, including execution status,
  provider/model, answer status, tool counts and evidence counts.

Model-assisted first-principles commands set an explicit cognitive mode instead of relying on
keyword inference. `/evidence`, `/audit` and `/report` do not call a provider and do not claim
that a model-generated statement is verified.

## Master mission and plan definition

The Workshop Master console adds the task-definition stage:

- `/mission new|show|revise|close`: maintain the active mission, workspace and revision;
- `/triage [goal]`: run deterministic route triage without invoking a provider;
- `/plan generate|show|revise`: generate a bounded PlanProposal, persist it under the
  session directory, validate its DAG and bind its SHA-256;
- `/approve [plan-id]`: approve the exact persisted plan digest;
- `/challenge`: in Master mode, defaults to challenging the bound plan.

Plan files remain `approved=false`; approval is a separate session-bound decision. This prevents
a model-generated plan from self-approving. Session reload validates that the plan stays inside
the session directory and still matches the recorded digest.

The composer also accepts natural language in Workshop Master mode. An actionable request
creates a Mission and displays a Goal/KR/task preview. Reply `确认计划` to approve the
exact plan and create `任务清单.md`; reply `确认执行` only after reviewing the prepared
task list. Explicit `/mission`, `/plan`, `/approve`, `/assign`, and `/dispatch` remain
available as the advanced command surface.

During supervision, `进度` / `任务板` reads the board, `待办` shows attention items,
`暂停督战` and `继续督战` control dispatch, and `补充任务 T3：…` targets an explicit
Work Item. Unrelated task intents are rejected rather than starting a generic model run.

## Agent inventory, assignment and dispatch

Phase 3 adds:

- `/model show|use|objective`: inspect provider manifests, pin the next provider/model route
  or change the route objective without bypassing capability filters;
- `/permissions show|network on|off|write on|off|root-add`: modify the next TUI run only,
  keeping read-only as the default;
- `/tools` and `/tools inspect <name>`: inspect the runtime ToolRegistry, risk, write need
  and JSON schema;
- `/agents list|refresh|inspect`: merge configured Agent profiles with Workshop Registry;
- `/assign show|apply`: run deterministic capability, tool, permission, risk and load
  filtering, then bind the resulting assignment to the approved plan;
- `/dispatch dry-run|run|detach`: materialize a durable MasterState from the session-bound
  plan and approval, then either show ready tasks or start Workshop Sessions with leases.

`/dispatch` does not create a second scheduler. It uses `MasterStore`, `MasterState` and
`WorkshopCliAdapter`; 1A owns approval and leases, while Workshop owns Session execution.

The packaged Linux binary provides the same command. An interactive Unix TTY uses Ratatui over
Crossterm, raw-mode input, mouse capture, bracketed paste and an alternate screen. Piped stdin or
stdout keeps the line-oriented interface used by scripts and CI.

## Supervision and human-in-the-loop control

After dispatch, the Workshop Master console adds:

- `/board [filter]`: render Work Item title, status, dependencies, Agent, Session, generation,
  lease expiry and last error from replay-validated `MasterState`;
- `/watch`: poll active Sessions using the bounded `[1A:RUNNING]`,
  `[1A:AWAITING_INPUT]` and `[1A:BLOCKED]` tail protocol, update task state and journal
  transitions or receipts;
- `/attention`: show operator questions, blocked/rejected/retrying/replanning tasks,
  `ArtifactReady` tasks awaiting verification and expired active leases;
- `/respond <task-id> <message>`: route an operator response to the task's Workshop
  Session and move `AwaitingInput` or `Blocked` back to `Running`;
- `/control pause|resume|cancel|takeover|return|retry|replan|reclaim`: apply explicit
  Master decisions. Cancel validates the task transition before stopping its Session;
  retry and replan also fail before Session cancellation when their state or bounded
  counters are invalid; reclaim requires an expired lease and a terminated execution;
  return ends a human takeover and emits `human_takeover.ended`;
- `/timeline`: show a bounded tail of Master journal events plus Workshop events, annotated
  with a matching Session ID when present.

These commands are visible only in Workshop Master mode. Standalone 1A and Worker mode
neither display nor execute them. `/watch` does not ingest raw Agent logs, and 1A does not
replace Workshop's Session scheduler.

## Artifact verification and production views

Phase 5 adds:

- `/artifacts [task-id|all]`: re-inspect expected files under each Work Item workspace and
  report presence, file type, size, actual/pinned SHA-256 and verdict;
- `/verify <task-id> <verification.json>`: submit exact criterion/test/evidence records;
  1A computes Artifact checks, requires a verifier different from the assigned Agent, updates
  MasterState and journals `Verified` or `Rejected`;
- Master `/evidence`: render acceptance criteria as claims bound to execution receipts,
  Artifacts, verifier evidence and failure reasons;
- Master `/audit`: replay the Master journal plus lease, generation, execution-target and
  current Artifact checks; `/audit all` also includes bounded run-journal audits;
- Master `/report`: render separate planned, executed, verified and decided sections.

After dispatch, `/evidence`, `/audit` and `/report` default to Master views. Explicit
`run` retains the Core run views. Artifact verification does not trust exit code or model
claims: missing delivery, criterion gaps, digest drift and self-verification fail closed.

## Sessions, workspace, context and presets

The Core surface now includes:

- `/session show|list|new|resume|fork|rename|archive|delete`;
- `/workspace show|cd|mention|diff|init|worktree`;
- `/context show|recap|compact`;
- `/preset list|save|show|run|delete`;
- `/cost`;
- `/config show|set|reset`;
- `/export session|report|evidence`;
- `/copy`.

`/status` displays an operator-facing, Chinese-language overview grouped into run state,
session/workspace, model/run limits and context/compaction. Technical identifiers such as
the session ID remain available, but raw `key=value` rows are not used as the primary
presentation. Long panels open at the top so the most important information is visible
first.

Context control is token-based. The default budget is 1,000,000 tokens with automatic
compaction at 786,432 tokens, matching the current Claude Code session. Runtime
compaction prunes old tool-result content when the context-token budget is exceeded.
The request-time count is a provider-neutral estimate; after every response the
provider-reported `usage.input_tokens` replaces it. `/context compact` locally
summarizes up to eight recent runs and uses that bounded summary instead of the normal
six-turn facts until the next run invalidates it. `/config
set context|compact|steps|tools|attempts|timeout` persists bounded values for subsequent
runs; `context` and `compact` take token counts. `/config reset` restores CLI defaults.

`/cost` rebuilds a session ledger from provider responses in run journals. It reports
input/output/total tokens, configured `cost_micros`, provider/model groups and recent run
groups. Provider-reported non-zero cost is preserved; otherwise cost is calculated from
the selected route manifest's input/output price metadata. Local zero-cost providers are
shown separately. The ledger excludes external Workshop Agents that do not write a 1A run
journal, and `cost_micros` is not a currency claim.

Session switching, forking, workspace changes, mentions, preferences and context
summaries persist in session metadata. Deletion requires the exact session ID twice and
moves the directory to `.trash`; it never recursively removes a broad path.

Workspace `cd` canonicalizes directories and keeps them under configured read roots.
`mention` persists bounded path references for subsequent known facts. `diff` and
worktree commands use `git` without a shell. `init` creates `AGENTS.md` with create-new
semantics and refuses to overwrite it.

In an interactive TTY, `/workspace` itself opens a selectable action menu. The "设置工作区"
action enters a second-level list of authorized roots; `Enter` applies the selected root,
`Esc` returns to the parent menu, and "输入路径" returns control to the composer with
`/workspace cd ` prepared. Selecting a root updates `cwd`, persisted session workspace and
the displayed panel state together. Other listed actions run diff, initialization or
worktree inspection without leaving the panel.

Presets are bounded local instructions and do not call a provider until `/preset run`.
Exports create new Markdown files without overwriting existing files. `/copy` uses the
same native clipboard or OSC 52 fallback as mouse-copy.

## Layout

```text
────────────────────────────────────────────────────────────
› Compare A and B under a fixed budget
structured transcript, wrapped to the terminal width and scrollable

╭─ • 模型思考 Thinking · 等待响应 · 第 1 步 ───── • ↑ 1.2k / ↓ 340 · 缓存 40% · 上下文 ─── 13% · 12s ────╮
│                                                                                                  │
│ › Compare A and B under a fixed budget                                                           │
│                                                                                                  │
╰─ ● 运行中 · deepseek-flash · 直接回答 · Ctrl+T 模式 ────────────────────────────────────────────╯
```

When the terminal is too narrow for the heading and metrics together, the metrics fold onto
their own row inside the same fence instead of being truncated:

```text
╭─ • 模型思考 Thinking · 等待响应 · 第 1 步 ─────────╮
│ ↑ 1.2k / ↓ 340 · 缓存 40% · 上下文 ─────── 13% · 12s │
│                                                     │
│ › Compare A and B under a fixed budget               │
│                                                     │
╰─ ● 运行中 · deepseek-flash · 直接回答 · Ctrl+T 模式 ───────────────╯
```

The whole terminal is filled with a dark-green backdrop `#06100A`, 80% of the input
panel background brightness and slightly darker than the panel. The input box is a dark-green panel whose background, border and panel
text are all dimmed by 50% (`#08140D`, `#3F644B`, `#487748`); input text stays white. Token metrics sit on the
right side of the top border while they fit, and otherwise fold to the row below it. The
input-box top title comes from the run journal; the bottom border
shows run status, the configured model, the current cognitive mode and `Ctrl+T`. The
context meter uses `─`, the same glyph as the fence border, and halves from 20 to 10 cells
when the panel folds, keeping the reported percentage unchanged. The
percentage is `tokens sent by the last model request / max_context_tokens`, the same measure
that triggers auto-compaction, and it refreshes after each completed run. Provider usage is
authoritative after a response; the request-time value is an estimate. While an answer
occupies the top of the transcript, a filled prompt bar keeps the associated user
instruction visible. It yields when the original user prompt or a newer visible prompt
provides that context itself. The bar is presentation-only and is not included in transcript
selection or copying.

The mode menu offers `自动` plus the five reachable modes. The applied row is marked `●`, the
cursor `▌`; `↑`/`↓` moves, `Enter` applies and `Esc` closes. `升级处理` is deliberately absent
because nothing produces it. A manual choice overrides content inference for later tasks until
`自动` is selected again. It changes rendering, risk and the execute authorization path; only the
four epistemic commands rewrite the prompt itself.

An empty transcript shows the responsive `1AGENT` welcome surface, session identity, start guidance
and command/resume hints. Operational status, mode, risk, provider and budget are available through
`/status` and the command drawer.

The wordmark keeps the leading `1` in bright green; `Agent` is deep green `#487748`. The
Chinese caption and the English prefix are deep green, and the `SKYNET.CLUB` domain is the
bright-green highlight. Its first row includes the same one-column leading pad as the lower
`1` rows.

The composer is a three-row visual container with the editable text vertically centered, inside
the fenced input panel. The panel top border reports the activity state; token/cache/context/
elapsed share that border on wide terminals and fold to an inner metrics row on narrow ones;
the bottom border reports completed/running state, model, session mode and `Ctrl+T`. Its
Codex-style `› ` prefix uses the dimmed panel green.

All primary TUI chrome and operator panels use Chinese labels. English technical names are
retained only when they identify a command, model/provider, file path or interoperability
contract.

Conversation and composer surfaces use a one-column inset from the terminal edge. User messages use
a full-width gray block with one blank padded row above and below. One additional normal-background
row follows that gray block, separating the user's message from the assistant answer. A one-row gap
separates the activity status from the composer. Running state uses a Braille spinner.

Markdown prose collapses repeated empty lines, removes leading/trailing prose padding and keeps at
most one visual gap between paragraphs or code blocks. A code block's title, blank lines, code body
and footer share one continuous dark-gray surface, followed by a single black gap before the next
section.

## Interaction

- type a task and press Enter to run it;
- type `/` to open the filtered command drawer;
- type a keyword such as `/验收`, `/证据` or `/老板` to search both commands and the
  48 built-in prompt supplements; selecting a prompt fills the composer and leaves
  `{{variables}}` for editing; the empty drawer starts with 3 featured prompts;
- after typing a task, add a space and `//` to open a prompt-only drawer; selecting a
  template merges it before or after the task and `Esc` restores the task;
- use `Up`/`Down` in the command drawer and `Tab` to complete the selected command;
- `Ctrl+Tab` opens the mode menu; `Shift+Tab` is the fallback on terminals that cannot report
  Ctrl+Tab separately;
- `Ctrl+O` opens or closes the run-details overlay, which includes the per-event process
  trace (requests, responses, tool calls and results);
- drag over input or transcript text to create an internal selection; releasing the mouse copies
  a non-empty selection automatically;
- while dragging a transcript selection at the top or bottom edge, the viewport continues scrolling
  every 70 ms so the selection can extend into off-screen content;
- a successful copy shows a bright-green `✓ 已复制选择内容` confirmation;
- `Ctrl+C` copies a selection, or cancels the active run when nothing is selected;
- `Ctrl+V` pastes through the host clipboard anywhere; right-click pastes only when
  the pointer is inside the composer;
- bracketed terminal paste is inserted as one edit and replaces a selection;
- `Left`, `Right`, `Home` and `End` edit the current input;
- `PageUp`, `PageDown` and the mouse wheel scroll the transcript;
- when scrolled away from the newest output, the activity area shows
  `↓ Back to bottom · esc`; new output changes it to
  `New activity · ↓ Back to bottom · esc`;
- press `Esc` or click the follow control to return to the newest output;
- hovering the follow control applies reverse-video highlighting;
- the sticky prompt bar tracks the first visible answer and yields at prompt boundaries;
- `Ctrl+Q` requests exit; press `Enter` to confirm or `Esc` to cancel, so a stray
  terminal key cannot close the session accidentally;
- `/help` shows commands;
- `/help` is an executable command palette rather than a static list; it is scoped to the
  current Core/Master surface and lifecycle stage, and `Enter` opens or runs the selected
  command;
- deterministic and production-control commands render in a replacement command panel rather
  than appending their output to the conversation transcript; starting another such command
  replaces the entire previous panel;
- each command panel exposes its own action menu. Actions without parameters execute through
  the normal command parser; actions requiring a path, task ID, model or value prepare the
  composer with the exact command prefix and return focus to input;
- when a command panel has selectable actions, `Up`/`Down` changes the selection and `Enter`
  opens or runs it; `Esc` returns to the parent panel, then to the conversation;
- read-only command panels use `PageUp`/`PageDown` to scroll their content;
- `/details` opens a scrollable full-screen run-details panel; `Esc` closes it and `Up`/`Down` or
  `PageUp`/`PageDown` scroll its content without adding messages to the transcript;
- `/journal` shows the latest run journal path;
- `/clear` clears the transcript and last instruction;
- `/exit` or `/quit` ends the session.

Only one run executes at a time. Ratatui diffs terminal buffers, the editing cursor moves by Unicode
grapheme boundaries, and East Asian text is measured by display width. Mouse capture keeps native
terminal selection from treating background padding as copyable text; 1A copies only its internal
text selection.

Direct-answer mode uses a conversation-first layout for every answer status: only the conclusion is
shown as normal prose. Evidence, caveats, flip conditions, model priors, operational constraints,
completion metadata and journal paths stay out of the primary transcript. Decision, evidence,
diagnosis and execution modes retain their explicit reasoning sections. `/details` exposes the
complete structured result on demand.

Markdown code and inferred code are shown as separate full-width dark blocks with a visible
`╭─ / │ / ╰────` fence and an optional language label. Besides standard triple-backtick fences,
the renderer recognizes a standalone language label followed by code, such as `c` followed by
`#include`, as well as obvious unlabelled code starts such as `#include`, `def`, `fn`, `import`
and `package`. Ordinary prose is not promoted merely because it contains a code-related word.
Markdown fence markers are never displayed.

Standalone code labels recognize 163 labels and aliases, including C, C++, C#, Java,
Kotlin, Scala, Swift, Dart, Python, Rust, Go, JavaScript, TypeScript, Ruby, PHP, Lua, Haskell,
Elixir, F#, OCaml, Clojure, Julia, Zig, Solidity, SQL, GraphQL, HTML/XML/SVG, CSS families, JSON,
YAML, TOML, Dockerfile, Makefile, CMake, Nix, HCL/Terraform, Protobuf, TeX, Vue, Svelte and common
shell families. The list also includes newer/tooling formats such as Gleam, Odin, ReScript, Elm,
WGSL/GLSL/HLSL, Verilog/SystemVerilog/VHDL, Starlark, Bicep, Astro, Handlebars and JSON5. A standard
fenced block accepts an arbitrary language label.

Markdown blockquotes (`> ...`) are grouped into a gray-gutter `▌ 引用` surface. Citation wording and
any attribution remain part of the quoted text. Quotes use a dim gray supplementary tone. Query
results and execution receipts are omitted from the primary transcript completely. Terminal cell
fonts cannot safely change size per region; `DIM` provides the equivalent lower-emphasis cue for
quotes.

When a run uses `read_file`, `search_files` or another tool, its journaled observation is included
only in the `/details` overlay. Search details show match counts, scanned-file counts and up to three
representative file locations with excerpts; raw JSON is never injected into the conversation.
Opening, scrolling or closing the overlay does not add transcript entries. Full raw content remains
available through the run journal.

## Transcript hierarchy and colors

The transcript follows Codex's semantic styling rather than applying one color to an entire answer.
Headings establish hierarchy, labels identify evidence quality, description text stays neutral and
metadata is visually de-emphasized.

| Role | Color |
|---|---|
| User prompt marker `›` | bright green, bold |
| Composer prompt marker `›` | bright green, unbolded |
| User prompt body | white on the input-panel dark green `#08140D` |
| Sticky prompt bar | white on the input-panel dark green `#08140D` |
| Section marker `▌` and accent text | ANSI-256 blue `75` (nearest to Codex accent `#63A8F8`), bold |
| Normal answer and description text | terminal default foreground |
| Verified claim `[已验证]` | deep green `#487748` text and label, unbolded |
| User-asserted or derived claim | deep green `#487748` text and label, unbolded |
| Assumption, model prior or unknown | deep green `#487748` text and label, unbolded |
| Conflict, failure or error | deep green `#487748` text and label, unbolded |
| Evidence marker `↳` | deep green `#487748` |
| Evidence source label | deep green `#487748` |
| Quote marker, `引用` label and quoted body | gray `245`, dim |
| Code block border `╭ │ ╰` | bright green, bold |
| Code language label | high-light blue `75`, bold |
| Code text | gray `252` |
| Unified code-block background | ANSI-256 dark gray `236` |
| Added diff line background (`+`) | ANSI-256 dark green `22` |
| Removed diff line background (`-`) | ANSI-256 deep red `52` |
| Code statements and keywords | light magenta, bold |
| Code functions and calls | yellow, bold |
| Code classes, built-in types and type names | cyan, bold |
| Code strings | light green |
| Code numbers | magenta |
| Code comments | dark gray, italic |
| Code import/include references | light cyan |
| Constraint label and bullets | deep green `#487748` |
| Work log text, labels and `⎿` results | deep green `#487748`, unbolded |
| Work log bullet `•` | bright green |
| Work log links (local paths and URLs) | bright green |
| System command output | normal answer foreground |
| Metadata and journal path | gray `245` |
| Details overlay surface | green `#0A1F12`, bright-green border |
| Command drawer background | green `#0A1F12` |
| Mode menu background | green `#0A1F12` |
| Mode menu selected row | green `#143D24`, bright-green bold label |
| Command drawer border | dim green `#3F644B` |
| Command drawer title | bright green, bold |
| Command name | bright green, bold |
| Command description | bright green |
| Selected command row | green `#143D24`, bold |
| Conclusion title `结论 Conclusion` | bright green, bold |
| `Ctrl+T` shortcut key `T` | bright green, bold |
| Selection | reverse video over the semantic color |
| Terminal backdrop | dark green `#06100A`, 80% brightness of the input panel |
| Composer background | dimmed deep green `#08140D` |
| Input-panel border | dimmed green `#3F644B` |
| Input-panel text and bottom border | dimmed green `#487748`; no context percentage (top meter only) |
| Context meter | `─` cells matching the fence border; dimmed green used, black empty; 20 cells wide, 10 narrow |
| Running activity | dimmed green, unbolded |

While running, the activity line shows `Working... <elapsed> · ↑<input> ↓<output> · cache <hit-rate>`
and `Ctrl+C 取消`. Elapsed time updates from the run clock; token counts and cache hit rate update as
provider responses are journaled. A provider that does not report cache statistics shows `cache —`.
Bright-green copy/paste confirmations remain distinct. `NO_COLOR=1` disables terminal colors through
Ratatui while preserving all glyphs and hierarchy.

## Sessions

Each bare TUI invocation starts a new session and prints its ID/continue command on exit.
`--session <id>` reopens an explicit session (creating it if needed). Completed runs are associated
with that session, and the transcript is reconstructed from their journals. New requests in a
reopened session receive a bounded summary of recent user requests and answer conclusions as
`known_facts`; the full journal remains the audit source.

Session metadata is version 2 while version 1 remains readable. Each session also has an
append-only `events.jsonl` with contiguous sequence numbers for host binding, accepted or
rejected commands, task submission and closure. This event log is separate from the model-visible
run journal.

After leaving the alternate screen, the terminal prints:

```text
 ░░      ░░░░░   ░░░░░░  ░░░░░░░ ░░░    ░░ ░░░░░░░░
▒▒▒     ▒▒   ▒▒ ▒▒       ▒▒      ▒▒▒▒   ▒▒    ▒▒
 ▒▒     ▒▒▒▒▒▒▒ ▒▒   ▒▒▒ ▒▒▒▒▒   ▒▒ ▒▒  ▒▒    ▒▒
 ▓▓     ▓▓   ▓▓ ▓▓    ▓▓ ▓▓      ▓▓  ▓▓ ▓▓    ▓▓
 ██     ██   ██  ██████  ███████ ██   ████    ██

  Session   Compare A and B
  Continue  1agent tui --session ses_...
```

The post-exit banner uses the same bright-green `1` and deep-green `Agent` wordmark as the
welcome surface.

A new bare `1agent tui` starts a new session. Run the printed command to restore the transcript:

```bash
1agent tui --session ses_...
```

In the current workspace, `/home/skynet/.local/bin/1agent` launches the release binary with the Pi
DeepSeek credential and `deepseek-flash`. The launcher references Pi's credential at runtime and
does not duplicate the secret.

## Safety

- user instructions are sanitized before display;
- model/progress text is sanitized before terminal rendering;
- ANSI escape sequences, bidi controls and zero-width controls cannot be injected through the
  displayed instruction or transcript;
- pasted and displayed controls are sanitized before editing or rendering;
- clipboard actions occur only after an explicit copy/paste gesture;
- provider credentials are not rendered;
- human replies require an existing task-bound Workshop Session;
- destructive control operations validate Master state before terminating a Session;
- Workshop timeline events remain bounded and are marked unassociated unless their payload
  contains a known Session ID;
- the existing read-only, network and journal policies remain unchanged.

## Deferred: spinning welcome mark

The welcome screen is static. A Codex-style spin was scoped on 2026-11-05 and
deliberately deferred:

- Codex renders a 3D field to Braille cells at 50 ms per frame, two rotations per
  7.2 s loop, with per-cell RGB and smoothstep easing; it pauses while hidden.
- Our mark is a flat hexagon, so the cheaper route is to bake N rotations of
  `assets/skynet-matrix.svg` into constants and play them once at 60 ms per frame.
- Rotating the double-line `1Agent` wordmark does not work: text art falls apart
  when turned. The badge would spin above the wordmark.
- Requires an idle redraw ticker in the interactive loop, which currently only
  ticks while a selection is auto-scrolling.

## Current limitations

- no token streaming;
- syntax highlighting is lexical rather than a full compiler/language-server semantic model;
- multiline strings and comments spanning wrapped display rows use simplified highlighting;
- transcript search and copy modes beyond mouse selection are not implemented;
- task input waits for Enter rather than supporting multiline composition;
- Windows uses the line-oriented fallback until a verified raw terminal backend is added;
- macOS raw-terminal behavior has not yet been verified on a target host.

These are presentation limitations; the underlying runtime contracts remain the same.
