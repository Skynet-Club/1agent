# Changelog

## 0.96.0 - 2026-11-10

- Added home layout v2 with key material under `keys/`, a v1-to-v2 migration,
  and `upgrade --rollback`.
- Added cross-process key locking and ledger reload for memory payloads, and
  SHA-256 snapshot locks for delegation/evolution stores.
- Added best-effort HTTPS alert webhooks for provider health from `run` and
  `serve`.
- Fixed standalone TUI turns opening a modal `/frame` next-action menu and small-talk turns calling tools.
- Added a single runtime home (`$ONEAGENT_HOME` or `$HOME/.1a`) with
  `init`, `doctor`, `export`, `restore`, `upgrade` and `uninstall`; exports use
  a SHA-256 manifest and exclude key/credential files.
- Added local key escrow/restore: AES-256-GCM under a separate recovery key,
  with tamper, overwrite and path-boundary checks.
- Added cross-process `flock` and stale-writer detection to the task-graph,
  memory, skill, goal and action journals; long-lived surfaces reload and return
  a retryable conflict instead of wedging.
- Added a restart-continuity scenario covering memory, goals, commitments,
  delegation and evolution, plus an installer for packaged binaries.
- Added an end-to-end skill lifecycle scenario from demonstration capture through
  static evaluation, activation and rollback; a failing candidate cannot replace
  the active version, and the journal replays after restart.
- Added write-gated terminal tools (`terminal_create`, `terminal_write`,
  `terminal_output`, `terminal_exec`, `terminal_destroy`) backed by Skynet
  Workshop PTY sessions. They are hidden from the model and denied by the
  registry in read-only runs.
- Added `--allow-write` (now a compatibility flag) and raised the default
  provider attempt budget to 8 so multi-step terminal workflows can reach a
  final answer.
- Terminal tools now fail with an actionable message when Skynet Workshop is
  missing; the manual and README document the Linux-only support matrix.
- Execute-mode authorization replies now name `/permissions write on` and
  `--read-only` instead of only stating the read-only default.
- File writes and terminal execution are now enabled by default for `run` and
  `tui`; `--read-only` disables them. `--allow-write` remains accepted as a
  compatibility flag.
- Added `write_file` and `edit_file` tools: bounded content, allowed-root
  containment, symlink rejection, overwrite protection and atomic replacement.
- Added `fetch_url`: public HTTPS-only reads with private/loopback address
  rejection, revalidated redirects, text-content checks, HTML extraction and a
  2 MiB cap; requires `--allow-network`.
- Added `web_search`: bounded, deduplicated public search results through Bing's
  RSS endpoint; provider rate limits surface as tool failures.
- Added a stdio MCP client (`--mcp-config`): initialize, tool discovery and
  tool calls; remote tools are named `mcp_<server>_<tool>` and are write-gated
  unless the server declares `readOnlyHint: true`.
- Added `spawn_agent`: bounded child runs with isolated journal/workspace,
  inherited write/network permissions and a depth cap of 2.
- Added persistent `schedule add/list/remove` and `daemon` (interval jobs,
  pid lock, isolated child runs, `--once`); `--trigger-file` adds mtime-based
  triggering. Health, failure alerts and routine binding are listed below.
- Added `1agent cost`: token/cost totals and p50/p95 latency per provider/model
  from run journals.
- Added a CLI e2e test proving `scripts/install.sh` installs a runnable binary
  into a custom prefix.
- Added `1agent digest`: recent run/stop-reason counts, schedules and next run
  time, without calling a model.
- Added agent-facing skill tools (`skill_list`, `skill_eval`, `skill_register`,
  `skill_activate`, `skill_rollback`) over the existing journaled registry.
- Added `retention` for run journals: dry-run by default, `--apply` deletes
  expired files and appends a receipt; memory/audit retention is separate.
- Added agent-facing delegation tools (`delegation_list`, `delegation_register`,
  `delegation_revoke`) over the persisted profile/revocation store.
- Added `provider-alerts` to summarize the persisted provider-health alert log.
- Added `browser_read`: headless Chrome rendering for public HTTPS pages,
  returning bounded extracted text; click/typing and screenshots remain future
  computer-use work.
- Added `notify` for operator webhook notifications (HTTPS or loopback HTTP);
  email/IM/calendar connectors remain future work.
- Added `memory consolidate`: duplicate active memories are grouped by scope +
  content hash; dry-run by default, `--apply` soft-forgets older duplicates.
- Added a file-backed context-graph cache keyed by a deterministic goal/task/
  memory fingerprint; `SurfaceService` can opt in with `with_context_cache`.
- Added limit/cursor pagination to Surface goal, commitment, skill and evolution
  ledger lists; goal/commitment pagination is covered by tests.
- Added `skill export` / `skill import`: portable skill bundles are pinned by a
  caller-supplied SHA-256; tampered bytes, mismatched manifest hashes, unknown
  schemas and duplicate versions are rejected. Every manifest is preflighted
  before the journal is touched, and bundles are capped at 4 MiB. Skill CLI
  defaults now honor `ONEAGENT_SKILL_JOURNAL`.
- Added Ed25519 skill-bundle signing: `skill keygen` writes 0600 key files,
  `skill export --signing-key` signs, and `skill import --public-key` verifies
  against the trusted key. Signed bundles are rejected without a key, unsigned
  bundles are rejected when a key is supplied, and tampering fails. A remote
  marketplace index and revocation remain future work.
- Added skill routines: `run --routine-skill NAME@VERSION` composes an active
  skill into the task, and `schedule add --routine-skill` lets the daemon run
  it. Inactive or version-mismatched skills are rejected; no new permissions
  are granted.
- Added `schedule health` with `ok`/`overdue`/`failing` states and a CLI test
  proving a daemon restarted after downtime catches up an overdue persisted job.
  Failure alerts are listed below; escalation and cross-restart deduplication
  remain future work.
- Extended `1agent cost` with per-task aggregation (objective, runs, tokens,
  cost, sorted by cost) and per-tool call/failure counts. A static dashboard is
  listed below; tool-level token attribution and a live service remain future
  work.
- Added `notify --format slack|discord` payload adapters for incoming-webhook
  integrations; delivery is still tested only against loopback, and email or
  calendar connectors are not implemented.
- Added `read_pdf`, which extracts local PDF text through the system
  `pdftotext`: read-root and symlink checks, a character cap, control-character
  rejection and page form-feed normalization. Image/scan OCR, DOCX/PPTX and
  voice understanding remain unimplemented.
- `daemon` now records and optionally webhooks a `schedule_run_failed` alert
  when a scheduled run fails or times out (`--alert-log`, `--alert-webhook`).
  Escalation, deduplication across restarts and overdue-job paging remain
  future watchdog work.
- `digest` now includes `schedule_health` (`ok`/`overdue`/`failing`) and alert
  counts by kind from an optional `--alert-log`, still without calling a model.
- Added `1agent dashboard`: a self-contained static HTML report over usage,
  latency, tasks, tools, schedule health and recent alerts. Dynamic text is
  escaped and a restrictive CSP is embedded; there is no live server.
- Audit export now supports `after_seq` pagination and optional record return
  (`--include-records` / `AuditFilter.include_records`); receipts remain the
  default so a query does not accidentally dump journal contents.

## 0.95.0 - 2026-11-04

- Added requirement regression as a first-class capability: `ReframeProposal` with
  fail-closed invariants, `reframe` and `receipt` commands, and journal events that
  bind a proposal to a human decision in one complete run.
- Added `/reframe` to the TUI: the model proposes, the options appear as a
  selectable menu, Enter records the decision, and the interface stays responsive
  with Ctrl+C cancellation.
- Added a raw text path to the model provider (`complete_text`) so non-answer
  replies no longer have to be recovered from an `InvalidFinal` error.
- Added an end-of-turn next-action menu derived from the command registry, with a
  default entry, and verified through a pseudo terminal.
- Added the Occam output contract: five style laws in the system prompt, a
  decision menu that deliberately has no default, and a language rule pinned to
  the request.
- Added evidence for the above: two independent 14-task style sets against
  pre-registered targets, two frozen held-out rounds for reframing, and a
  claims-to-evidence table enforced by `verify.sh`.
- Added a participation kit and self-contained labelling sheets so external
  people can produce the semantic evidence that is still missing.
- Did not include the multi-goal production line; that program remains at M2 and
  is tracked separately.

## 0.94.0 - 2026-11-03

- Completed the phased 1A production-control command surface, including mission and
  approval, dispatch and supervision, independent Artifact verification, context
  headroom/compaction, session/workspace/preset controls and a full TUI workflow from
  planning through verified reporting.
- Added `/cost`, a journal-derived token/cost ledger with provider-reported cost
  preservation and route-manifest pricing fallback.
- Added automatic context compaction at a configurable 50%-95% threshold, defaulting to
  80%, with persistence and legacy session compatibility.
- Added `master capability-probe` with versioned cross-runtime tasks, bounded commands,
  pass/fail/blocked classification and hash-only output evidence.
- Added runtime-filtered probes and 1A-native DeepSeek JSON-path validation.
- TUI provider-turn budgets now default to `--max-steps` and can be overridden,
  preventing multi-step tool use from ending before a final answer.
- TUI command results now use mutually exclusive replacement panels instead of appending
  command output to the conversation; `Esc` returns to the transcript and panel scrolling
  is isolated from transcript scrolling.
- List-bearing command panels now support keyboard-selectable submenus. `/workspace`
  opens an action menu, its workspace action lists authorized roots, and Enter applies a
  selected root while `Esc` returns to the parent menu.
- Operator-facing command output now uses grouped Chinese labels instead of raw key/value
  rows. `/status`, `/context`, `/config`, `/permissions`, `/model`, `/tools`, `/agents`,
  `/cost`, session and preset views, command-drawer labels and TUI hints share the same
  user-first presentation.
- Every panel command now exposes selectable actions. `/help` is a live command palette:
  `Enter` executes the selected command, actions that need arguments prepare the composer,
  and command panels can execute related actions without retyping slash commands.
- Added host-aware TUI command scopes: standalone and Worker modes cannot see or execute
  Workshop Master-only commands, and Master commands are filtered by lifecycle stage.
- Added versioned TUI session event logging with v1 compatibility, contiguous event
  sequences and explicit command rejection records.
- Added first-principles TUI commands for framing, challenge, decision analysis,
  postmortem, evidence inspection, journal audit and bounded run reporting.
- First-principles analysis commands now select an explicit cognitive mode and persist it
  in the run journal; evidence and audit views are deterministic and provider-free.
- Added Workshop Master mission definition commands for mission lifecycle, deterministic
  triage, validated plan generation/revision and digest-bound plan approval.
- Added session v2 mission and plan bindings with path containment and plan-digest checks
  on reload.
- Added TUI model-route inspection/selection, per-run network/write/read-root controls and
  ToolRegistry inspection.
- Added Workshop Agent discovery, deterministic assignment and the session-to-Master
  dispatch bridge with leases, idempotency keys and controlled Session execution.
- Added Master supervision commands for the Work Item board, bounded Session polling,
  attention filtering and human response routing.
- Added explicit journaled controls for pause, resume, cancel, human takeover, one bounded
  retry and one affected-subgraph replan; invalid controls are rejected before a live
  Session is terminated.
- Added a bounded Master/Workshop timeline with Session-ID correlation when the event
  payload provides one.
- Added Master `/artifacts` and `/verify` with current-filesystem checks, independent
  criterion evidence, verifier identity separation and journaled acceptance state.
- Added Master evidence, production report and audit views bound to claims, receipts,
  Artifacts and verifier evidence; audit also detects current Artifact drift.
- Added `/context` character-budget headroom, deterministic local recap/compaction and
  persisted `/config` controls for steps, tool calls, provider attempts, context and timeout.
- Added session list/new/resume/fork/rename/archive and recoverable delete, persisted
  workspace and mentions, bounded presets, session/report/evidence export and `/copy`.
- Added workspace Git diff/worktree commands and create-new `AGENTS.md` initialization.
- Added 1A Master-Agent V1: deterministic triage, bounded plan generation, DAG
  validation, capability-based Agent assignment, dry-run approval and scoped execution.
- Added a persisted Master state machine for task leases, idempotency keys, execution
  targets, supervision, human takeover, cancellation, one automatic retry and one
  affected-subgraph replan.
- Added independent Artifact checks with workspace path containment, regular-file
  validation, streaming SHA-256 and exact acceptance-criterion coverage.
- Added append-only Master Journal events with full state snapshots, contiguous sequence
  auditing and replay-based loading.
- Added planner retry for one invalid/tool-call response and private per-plan JSONL
  records of every model-visible planning request, response and normalized error.
- Added a bounded Skynet Workshop CLI adapter for Agent/Registry, Job, event and controlled
  Agent execution reads, while tolerating forward-compatible response fields.
- Added parallel dispatch up to the plan's `max_parallel` through Workshop `agent-run`,
  with one persistent Session per Work Item.
- Added detached persistent-Session execution and `master poll`, mapping real Session
  running/completed/timed-out states back into the Master state machine.
- Added an explicit `[1A:AWAITING_INPUT]`, `[1A:RUNNING]`, `[1A:BLOCKED]` tail-marker
  protocol for bounded Agent supervision without ingesting raw logs.
- Added reject and versioned revise workflows for plans that have not started.
- Required every new Master state to initialize unapproved; a plan file cannot self-approve.
- Added a false-success pilot: a command exits 0 while omitting the expected Artifact,
  and independent verification rejects completion with `artifact is missing`.
- Made the TUI safe to launch before a Workshop GUI attaches by removing a redundant
  cursor-position query after entering the already-blank alternate screen.
- Added Workshop `1-Agent` runtime discoverability and validated the real native TUI
  inside the Master-Agent pane.
- Added the `master` command family for triage, planning, approval, execution, verification,
  supervision, recovery, platform reads and audit.
- Verified a real Workshop execution: Session `term-1793636352398-530889-18`, verified
  `result.txt`, seven audited Master events.
- Recorded same-task Pi/Hermes/Claude/Codex capability probe blockers without promoting
  failed or unmeasured runs to success rates.

## 0.92.0 - 2026-11-02

- Replaced the handwritten ANSI screen painter with a Ratatui/Crossterm interaction layer.
- Added grapheme-aware text editing, bracketed paste, mouse capture and internal selection.
- Added automatic copy when a mouse selection is released, `Ctrl+C` selection copy through the
  native clipboard or OSC 52, plus `Ctrl+V` and right-click pasting.
- Added mouse-selection edge autoscroll: holding the drag at the top or bottom of the transcript
  continuously scrolls and extends the selection into off-screen content.
- Matched the Codex `›` composer prompt with a two-column prefix.
- Applied the Codex-blue `ANSI 75` highlight color to the composer `›` prefix.
- Added semantic transcript hierarchy and color tones for headings, verified/derived/assumed
  claims, evidence, warnings, failures, descriptions and metadata.
- Removed the global status row from the interactive surface; status is available through
  `/status`, while the empty transcript serves as the welcome surface.
- Added a bright-green confirmation message after mouse-selection copy succeeds.
- Added a Codex-style transcript follow control with `New activity`, `Back to bottom` and Esc.
- Added reverse-video hover feedback for the transcript follow control.
- Added persistent TUI sessions, journal-backed transcript restoration and a post-exit
  `Session` / `Continue` summary.
- Injected a bounded summary of recent session turns into `known_facts` so continued requests
  retain conversational context rather than only restoring the visible transcript.
- Added a responsive 1-Agent ASCII welcome surface with session identity, start guidance and
  command/resume hints.
- Styled the running spinner bright green, the `正在运行` label bright blue and the remaining
  activity hints muted gray.
- Replaced the welcome mark with the user-selected eight-row box-drawing wordmark.
- Highlighted the leading `1` glyph in bright green while keeping its two left-edge bars blue.
- Replaced the welcome wordmark with the selected five-row shade-gradient `1Agent` design.
- Aligned the first wordmark row with the vertical stem of the leading `1`.
- Reused the colored welcome wordmark in the post-exit Session/Continue banner.
- Added a Pi-backed `1agent` launcher that reads the existing Pi DeepSeek credential at runtime
  without copying the secret into configuration.
- Added a DeepSeek `deepseek-flash` Provider profile and tightened read-only
  `action_receipts` output rules.
- Changed TUI answers to conversation-first adaptive rendering: direct answers stay concise,
  while decision/evidence tasks retain the full structured reasoning hierarchy.
- Moved repetitive schema, stop reason and journal information to `/details` and `/journal`.
- Kept completed direct answers to plain conversational prose by default; evidence and caveats
  remain available through `/details`.
- Clarified that `model_prior` claims must have an empty `sources` array and added a prompt-contract
  regression test.
- Required unknown evidence timestamps to use `null`, rejected `0` placeholders and required
  expiration to follow observation when timestamps are present.
- Added a one-shot model-output repair pass for both non-JSON final text and domain-validation
  failures; the runtime preserves the original response in the journal, returns the precise error
  to the same provider and revalidates its corrected JSON.
- Required every evidence source to exactly match the sources declared by each referenced claim.
- Added Markdown and inferred-code parsing with visible `╭─ / │ / ╰────` fences, language labels
  and full-width dark code surfaces; standalone language labels and obvious unlabelled code no
  longer require triple-backtick fences.
- Styled code-fence borders in bright green and code-language labels in the high-light blue accent
  while preserving the gray code text and dark code surface.
- Added lightweight language-aware syntax highlighting for statements, keywords, functions,
  classes/types, strings, numbers, comments and import/include references across C-family,
  Python, Rust, JavaScript/TypeScript, Java/Kotlin/Scala, Go, shell and SQL code blocks.
- Expanded bare code-label recognition to 163 language and format aliases, including
  Kotlin, C#, Swift, Ruby, PHP, Haskell, Solidity, Dockerfile, Nix, TeX, Vue, Protobuf and
  GraphQL.
- Added a dedicated `▌ 引用` transcript surface for Markdown blockquotes.
- Added human-readable `查阅结果` entries backed by actual journaled tool observations. Search
  results show match counts, file locations and excerpts instead of raw JSON; source/status labels,
  a distinct background and a bounded display size remain visible.
- Rendered quotes and query results as lower-emphasis gray supplementary content using `DIM` to
  distinguish them from the primary answer without unsafe per-region font-size changes.
- Removed tool/query receipts from the primary transcript entirely. `/details` now opens a
  scrollable overlay containing run structure and human-readable tool results; closing it does not
  append anything to the conversation.
- Fixed space input in the composer.
- Added inset conversation/input surfaces, full-width user-message backgrounds with vertical
  padding, localized activity labels, extra status-to-composer spacing and a Braille run spinner.
- Standardized the product name as `1-Agent`, including the welcome logo, runtime identity,
  system prompt and terminal session summary.
- Added a filtered `/` command drawer with keyboard selection and Tab completion.
- Added a Codex-style sticky prompt bar that follows the visible transcript and yields when the
  corresponding user prompt is already on screen.
- Added structured transcript rendering with width-aware wrapping, mouse selection and scrolling.
- Added a blank normal-background row after the user message's gray padding so the assistant answer
  starts with visible vertical separation.
- Compacted repeated Markdown blank lines and unified each code block's title, blank lines, code
  body and footer onto one continuous dark-gray surface, one shade lighter than the terminal
  background so the block remains visible.
- Added Codex-style diff backgrounds: green rows for `+` additions, deep-red rows for `-` deletions
  and a neutral dark surface for unchanged code.
- Kept the line-oriented fallback for pipes and non-interactive hosts.
- Recorded OpenAI Codex and Ratatui attribution for reused interaction concepts and algorithms.

## 0.91.0 - 2026-11-02

- Added an interactive `1agent tui` command.
- Added a fixed topbar that preserves the latest user instruction.
- Displayed status, cognitive mode, risk, provider identity and run budget.
- Added `/help`, `/clear`, `/exit` and `/quit`.
- Cancelled active runs with Ctrl+C while preserving the session.
- Sanitized terminal-bound input and output against control-sequence injection.
- Added view-model and executable CLI E2E coverage.

## 0.90.0 - 2026-11-02

- Made provider readiness evaluate credential-bearing transport rules.
- Prevented remote HTTP providers with declared keys from appearing ready.
- Kept deterministic and local unauthenticated provider readiness unchanged.
- Added readiness/transport consistency coverage.

## 0.89.0 - 2026-11-02

- Rejected bearer credentials sent to non-loopback plaintext HTTP endpoints.
- Kept plaintext HTTP for local loopback providers without credentials.
- Rejected empty API keys and credentials embedded in endpoint URLs.
- Added provider-construction coverage for remote and loopback endpoints.

## 0.88.0 - 2026-11-02

- Added direct decision-completeness floor status to human-readable report summaries.
- Prevented readers from having to compare the completion rate with the floor manually.
- Kept the machine-readable summary contract unchanged.
- Extended signed-report text-output E2E assertions.

## 0.87.0 - 2026-11-02

- Added explicit integrity, content-binding, signature and authenticity labels to concise text summaries.
- Kept machine-readable summary fields unchanged.
- Added signed-report text-output CLI coverage.

## 0.86.0 - 2026-11-02

- Rejected partial approval proof arguments before reading or validating a report.
- Replaced unchecked summary/report count additions with checked arithmetic.
- Added a CLI E2E that proves proof-argument validation precedes file access.
- Added an overflow-boundary summary test.

## 0.85.0 - 2026-11-02

- Added CLI end-to-end rejection of a report approval with the wrong artifact kind.
- Added CLI end-to-end rejection after the report signing key is revoked.
- Kept informational and successful authenticated inspection behavior unchanged.
- Extended the report trust audit with partial-proof and registry-state negatives.

## 0.84.0 - 2026-11-02

- Added HMAC-authenticated evaluation report inspection through approval envelopes.
- Bound report signatures to the exact report bytes and the evaluation-report artifact kind.
- Enforced approval key registry state during authenticity verification.
- Distinguished signature presence from verified authenticity in summaries.
- Added signed-report CLI end-to-end coverage and honest symmetric-key limits.

## 0.83.0 - 2026-11-02

- Added explicit content-binding and authenticity trust requirements for report consumers.
- Kept informational inspection compatible with unbound reports.
- Rejected unbound reports when content binding is required.
- Rejected all reports when authenticity is required because signatures are not implemented.
- Added trust-policy unit and CLI end-to-end coverage.

## 0.82.0 - 2026-11-02

- Added an explicit three-part verification summary to evaluation reports.
- Distinguished checked integrity, content binding and authenticity.
- Marked all current summaries as unauthenticated.
- Advanced the summary schema to version `3` for required verification metadata.
- Validated binding consistency and current-summary integrity checks.

## 0.81.0 - 2026-11-02

- Added canonical SHA-256 digests to evaluation reports.
- Bound compact summaries to the normalized report content digest.
- Advanced the summary schema to version `2` while accepting versions `0` and `1`.
- Rejected changed case payloads whose retained digest no longer matches.
- Preserved decoding for reports and summaries that predate content digests.

## 0.80.0 - 2026-11-02

- Added schema version `1` to evaluation report summaries.
- Validated summary counts, basis-point ranges and floor status.
- Decoded version-less version `0` summaries while normalizing new output.
- Rejected unsupported summary versions and internally inconsistent summaries.
- Added legacy, boundary and CLI end-to-end summary-version coverage.

## 0.79.0 - 2026-11-02

- Added compact `eval-inspect --summary` output for host integrations.
- Excluded case payloads and case identifiers from the summary contract.
- Preserved explicit full normalized report output.
- Applied existing report-integrity validation before summary generation.
- Added unit and CLI end-to-end summary coverage.

## 0.78.0 - 2026-11-02

- Bound evaluation report inspection to internal aggregate invariants.
- Recompute case, pass/fail, hard-gate and rubric totals before acceptance.
- Recompute decision-completeness aggregates and floor status before acceptance.
- Reject reports whose top-level claims do not match their case records.
- Added tampered-report unit and CLI end-to-end coverage.

## 0.77.0 - 2026-11-02

- Added `eval-inspect` for external evaluation reports.
- Validated supported decision-completeness policy versions and source combinations.
- Preserved legacy version `0` reports while normalizing JSON output.
- Rejected inconsistent metadata and unsupported versions before inspection.
- Added unit and CLI end-to-end coverage for valid and invalid inspection.

## 0.76.0 - 2026-11-02

- Added decision-completeness policy version `1` to evaluation reports.
- Recorded whether the effective floor came from suite defaults or explicit configuration.
- Decoded reports without policy metadata as legacy policy version `0`.
- Rejected unknown floor-source metadata.
- Added unit and CLI end-to-end coverage for policy provenance.

## 0.75.0 - 2026-11-02

- Added optional `min_decision_completeness_bps` to evaluation suites.
- Default the floor to 10,000 bps for model-quality suites.
- Default other suites to zero.
- Reject configured values above 10,000 bps.
- Add floor boundary and kind-aware default coverage.

## 0.74.0 - 2026-11-02

- Added suite-level decision-completeness aggregation.
- Added completion rate in basis points.
- Aggregated conditional, bound/unbound flip and high-impact gap counts.
- Added a 100% decision-completeness floor for model-quality suites.
- Surface floor status in JSON and human-readable evaluation reports.
- Make CLI evaluation exit non-zero when the floor fails.
- Preserve contract-fixture behavior.

## 0.73.0 - 2026-11-02

- Added per-case decision-completeness rollups.
- Count conditional answers with bound flip conditions.
- Count bound and unbound flip conditions separately.
- Count high-impact unknowns without reduction methods.
- Detect completed answers depending on high-impact assumptions.
- Treat decision-completeness failures as hard-gate failures.
- Display decision metrics in human-readable evaluation output.
- Added direct rollup coverage for each failure mode.

## 0.72.0 - 2026-11-02

- Added a concise human-readable experience audit summary.
- Display event, candidate, active and superseded counts.
- Display supersession chain count, depth and expired Active count.
- Preserve the stable JSON audit schema.
- Added CLI text-output coverage.

## 0.71.0 - 2026-11-02

- Added experience status counts to audit reports.
- Added supersession chain count and maximum depth.
- Detected orphaned supersession references.
- Detected supersession cycles.
- Reported expired Active experiences as audit failures.
- Added lifecycle, cycle and expiration coverage.

## 0.70.0 - 2026-11-02

- Added machine-readable experience-store audit reports.
- Added stable finding codes for I/O, JSON, sequence, size, timestamp, candidate,
  transition, evidence, metadata and supersession failures.
- Added `experience audit`.
- Added JSON output and CLI end-to-end coverage.
- Kept candidate listing unchanged for valid stores.

## 0.69.0 - 2026-11-02

- Capped experience actor text at 128 characters.
- Capped transition reasons and verification evidence at 1,000 characters.
- Capped each experience JSONL event at 64 KiB.
- Rejected non-monotonic experience event timestamps.
- Validated transition controls and bounds during creation and replay.
- Added oversized-line, oversized-actor and timestamp-regression coverage.

## 0.68.0 - 2026-11-02

- Capped experience title at 200 characters.
- Capped experience content at 4,000 characters.
- Capped experience scope at 200 characters.
- Capped evidence references at 32 per proposal.
- Required non-empty, unique applicable cognitive modes.
- Applied the same bounds during proposal and experience replay.
- Added boundary and invalid-shape coverage.

## 0.67.0 - 2026-11-02

- Reject proposed experience events that are not unverified drafts.
- Validate candidate version, source run ID, mode list and evidence references on replay.
- Validate candidate evidence digest format while preserving legacy digest-free entries.
- Recheck supersession version increments and active-scope uniqueness during replay.
- Added tampered experience-history coverage.

## 0.66.0 - 2026-11-02

- Restricted experience evidence to observation, answer and provider-failure events.
- Rejected lifecycle-only events such as run start, routing and checkpoints.
- Added a stable invalid-evidence-kind error.
- Covered accepted answer evidence and rejected lifecycle evidence.
- Made experience E2E derive evidence sequence from the source journal.

## 0.65.0 - 2026-11-02

- Added a canonical digest over referenced experience evidence events.
- Stored the evidence digest on candidates.
- Required duplicate evidence sequences to be rejected.
- Required source events for review, approval and activation of digest-bound candidates.
- Revalidated source audit, run identity and event-content digest.
- Added `--journal` to experience review, approve and activate.
- Preserved legacy candidates with no evidence digest.
- Added rewritten-source detection and CLI coverage.

## 0.64.0 - 2026-11-02

- Added the `superseded` experience status.
- Required superseded candidates to match the prior active kind and scope.
- Incremented experience version from the superseded candidate.
- Atomically transitioned prior Active to Superseded during activation.
- Rejected competing Active candidates in the same kind and scope.
- Rejected expired proposal and activation.
- Preserved superseded status after replacement rollback.
- Added supersession, conflict and expiration tests.

## 0.63.0 - 2026-11-02

- Added replayable retry timing outcomes with requested and elapsed duration.
- Distinguished completed and cancelled retry delays.
- Added a bounded 2-second elapsed timing audit.
- Detected timing without a preceding schedule.
- Detected timing whose delay differs from its schedule.
- Preserved cancellation as a legitimate terminal retry outcome.

## 0.62.0 - 2026-11-02

- Shared the 1-second provider retry backoff cap through the domain contract.
- Audited retry targets against the active route plan.
- Audited the next selected route against the scheduled target.
- Detected delay values above the hard cap.
- Detected duplicate and stalled retry schedules.
- Preserved cancellation and budget exhaustion as legitimate reasons to abandon a schedule.
- Added stable finding codes for retry contradictions.

## 0.61.0 - 2026-11-02

- Added deterministic exponential backoff between retryable provider failures.
- Added `provider_retry_backoff_ms` with legacy default `50`.
- Added `--provider-retry-backoff-ms`.
- Capped delay at 1 second and supported explicit zero-delay operation.
- Check remaining run time before scheduling or sleeping.
- Journal and replay provider retry delay decisions.
- Make backoff cancellation-responsive.
- Added boundary, cancellation and zero-delay tests.

## 0.60.0 - 2026-11-02

- Added provider failure count and unique stable codes to route traces.
- Added per-run `max_provider_attempts` budget with a hard 16-attempt ceiling.
- Added `--max-provider-attempts`.
- Preserved consumed provider attempts in checkpoints and resume.
- Emitted `BudgetExhausted` with a `needs_evidence` answer when fallback is blocked by budget.
- Fixed audit acceptance for the legitimate budget-exhausted/needs-evidence combination.
- Added fallback-budget and checkpoint coverage.

## 0.59.0 - 2026-11-02

- Added a structured `provider_failed` journal event.
- Record provider/model, error code, retryability and a bounded sanitized message.
- Preserve provider failures in replay state.
- Audit failures that do not match the selected route.
- Audit contradictory success followed by failure in the same attempt.
- Updated retryable and non-retryable runtime coverage.

## 0.58.0 - 2026-11-02

- Removed HTTP client error formatting from provider transport errors.
- Kept timeout and network classifications without exposing endpoint or query values.
- Made transport retryability depend only on timeout or connect failure.
- Added runtime coverage proving authentication failures do not trigger fallback.
- Retained existing coverage proving retryable failures trigger fallback.

## 0.57.0 - 2026-11-02

- Stopped copying non-2xx provider response bodies into errors.
- Classified HTTP errors by status before consuming their bodies.
- Made authentication and malformed-request failures non-retryable.
- Kept rate-limit, timeout and server errors retryable.
- Exposed only normalized status metadata in provider HTTP errors.
- Covered authentication, rate-limit, gateway timeout, server and bad-request responses.

## 0.56.0 - 2026-11-02

- Added an 8 MiB response-body ceiling for OpenAI-compatible providers.
- Rejected oversized declared `Content-Length` before body consumption.
- Bounded streamed and chunked response bodies during accumulation.
- Added the stable `response_too_large` provider error code.
- Applied the same response ceiling to successful and HTTP-error bodies.
- Ensured size errors omit response content.

## 0.55.0 - 2026-11-02

- Added stable machine-readable journal audit finding codes.
- Associated findings with event indexes where a concrete event exists.
- Preserved the existing human-readable audit `issues` list.
- Covered unsupported protocol state, incomplete decisions, duplicate answers and
  unpaired tool calls with stable codes.
- Made audit findings readable when decoding older audit reports.

## 0.54.0 - 2026-11-02

- Upgraded new answers to schema version 2 while retaining version 1 validation.
- Added stable IDs to unknowns in the current answer contract.
- Required conditional answers to include at least one flip condition.
- Rejected completed answers that depend on high-impact assumptions.
- Bound flip conditions to declared claims or unknowns.
- Rejected empty, duplicate or dangling flip-condition targets.
- Preserved legacy unversioned answers as version 1 with their original semantics.
- Added domain, journal audit and provider contract coverage.

## 0.53.0 - 2026-11-02

- Added an explicit schema version to structured agent answers.
- Preserved decoding of legacy unversioned answers as schema version 1.
- Rejected unsupported future answer schema versions before semantic validation.
- Exposed the answer schema version in journal audit reports.
- Updated provider and fixture contracts to emit versioned answers.
- Added legacy migration and fail-closed version tests.

## 0.52.0 - 2026-11-02

- Added optional observation and expiration timestamps to evidence.
- Rejected expired evidence supporting verified claims.
- Added task-level maximum evidence age policy.
- Required observation timestamps when a freshness policy is active.
- Added `--evidence-max-age-ms` for run tasks.
- Added stale evidence counts to JSON scorecards.
- Added freshness window, stale and missing-timestamp tests.

## 0.51.0 - 2026-11-02

- Bound conflict resolution sources to declared claim provenance.
- Added optional resolution claim IDs distinct from conflict sides.
- Rejected resolution sources absent from side or resolution claims.
- Preserved unresolved conflicts with no resolution claim metadata.
- Updated provider contract and resolution provenance tests.

## 0.50.0 - 2026-11-02

- Added explicit conflict records with two claim sides.
- Added unresolved and resolved conflict states.
- Required resolved conflicts to include resolution text and source evidence.
- Rejected resolved conflicts that still contain conflicting claims.
- Required every conflicting claim to have an open unresolved conflict.
- Added unresolved-conflict minimums and counts to evaluation scorecards.
- Updated provider contract and scripted conflict fixture.

## 0.49.0 - 2026-11-02

- Upgraded assumptions to structured records with description, impact and verification method.
- Preserved decoding of legacy string assumptions as medium impact.
- Required high-impact assumptions to provide a verification method.
- Added high-impact assumption minimums to evaluation cases.
- Added assumption counts by impact to JSON scorecards.
- Updated human-readable output and provider prompt schema.

## 0.48.0 - 2026-11-02

- Added low, medium and high impact levels to unknown items.
- Required high-impact unknowns to provide a reduction method.
- Added high-impact unknown minimums to evaluation cases.
- Added low/medium/high unknown counts to JSON scorecards.
- Preserved string assumptions for backward compatibility.

## 0.47.0 - 2026-11-02

- Verified claims now require a non-empty source.
- Derived and assumed claims now require a non-empty scope.
- Model-prior claims cannot declare external evidence sources.
- Conflicting claims require at least two distinct sources.
- Evidence must be non-empty and reference at least one claim.
- Evidence claim references must be unique.
- Evidence source must already be declared by every referenced claim.
- Added provenance invariant tests used by runtime and journal audit.

## 0.46.0 - 2026-11-02

- Added evaluation execution statuses: unspecified, template, draft and ready.
- Added provider requirements, reviewer and calibration prerequisites.
- Added `evals/model-quality-template.json`.
- Allowed templates with no cases to load but prevented execution.
- Required ready status and metadata for runnable model-quality suites.
- Prevented template/draft inspection from being treated as completed evidence.
- Added template load/run rejection coverage.

## 0.45.0 - 2026-11-02

- Added suite kinds: unspecified, contract fixture and model quality.
- Displayed suite kind in JSON and text evaluation reports.
- Marked core and shipped held-out examples as contract fixtures.
- Prevented contract fixtures from being promoted as model-quality experience evidence.
- Preserved compatibility for existing suites without a kind.

## 0.44.0 - 2026-11-02

- Added a scripted reasoning fixture provider for evaluation-framework validation.
- Covered contradictory sources with explicit conflicting claims.
- Covered conditional decisions with flip conditions.
- Covered self-correction only after a real file observation.
- Clearly separated fixture behavior from real-model quality evidence.

## 0.43.0 - 2026-11-02

- Added minimum unknown and flip-condition expectations to evaluation cases.
- Added unknown/flip-condition counts to JSON scorecards.
- Expanded the core suite from four to six cases.
- Added missing-file evidence failure coverage.
- Added zero-width hidden-control rejection distinct from bidirectional controls.
- Upgraded the suite to `core-v2` with explicit suite version `2`.

## 0.42.0 - 2026-11-02

- Added compact runtime tool-capability snapshots with schema digest and tool names.
- Journaled the snapshot before execution.
- Bound checkpoints to the snapshot digest.
- Rejected resume when tool schemas or capability metadata change.
- Preserved legacy checkpoints without a snapshot.
- Exposed the tool snapshot digest through resume status.
- Added replay and changed-registry resume coverage.

## 0.41.0 - 2026-11-02

- Added `tools --json` for host-visible capability inspection.
- Reports tool names, descriptions, schemas, risk, write requirement, timeout and status.
- Uses the same runtime registry/spec source as execution.
- Added implemented/degraded/unsupported capability status taxonomy.
- Added end-to-end coverage for tool inventory and path redaction.

## 0.40.0 - 2026-11-02

- Added central observation summary/content bounds.
- Preserved truncation metadata and a general `truncated` marker.
- Rejected hidden/bidirectional controls across the final observation boundary.
- Preserved structured observation status and error codes.
- Added adversarial oversized and unsafe observation coverage.

## 0.39.0 - 2026-11-02

- Added a bounded JSON Schema subset validator at the central tool boundary.
- Validates object shape, required fields, additional properties, base types, string/integer
  bounds, array items and enums.
- Rejects invalid arguments before tool-specific execution.
- Kept path containment, side effects, permissions and concurrency checks inside each tool.
- Added nested-schema and unexpected-field coverage.

## 0.38.0 - 2026-11-02

- Added OpenAI-compatible assistant content normalization for strings and text-part arrays.
- Rejected unsupported or malformed content parts.
- Assigned deterministic IDs to missing tool-call IDs.
- Rejected duplicate, empty or malformed tool names and duplicate call IDs.
- Kept provider wire types isolated from the domain/tool contracts.
- Added adapter normalization tests.

## 0.37.0 - 2026-11-02

- Bounded human-readable route reasons to 240 characters.
- Added first-model-step fallback-attempt count to route summaries.
- Moved route-summary formatting into a testable library function.
- Added bounded-output and fallback-count coverage.
- Left complete, untruncated reasons in journal/replay JSON.

## 0.36.0 - 2026-11-02

- Added concise route summaries to non-JSON run and resume output.
- Displayed mode, objective, candidate/rejection counts, first selected route and its reason.
- Displayed policy-source attribution.
- Kept full plans in JSON journal/replay output.
- Ensured traces omit endpoints, credentials and prompt content.
- Added route-trace summary coverage.

## 0.35.0 - 2026-11-02

- Added `resume-status` for inspecting checkpoint route metadata without resuming.
- Added explicit route-aware versus legacy checkpoint status.
- Resume now reuses the checkpoint objective when no CLI override is provided.
- Explicit conflicting objective overrides fail before provider execution.
- Preserved stored policy-source attribution during resume.
- Added CLI coverage for status, automatic objective recovery and conflicting override rejection.

## 0.34.0 - 2026-11-02

- Bound checkpoints to the canonical route plan and policy-source digest.
- Persisted effective route policy sources in each checkpoint.
- Resume recomputes the plan and rejects changed objective, budgets, provider manifests or policy
  sources.
- Preserved legacy checkpoints without route digests.
- Added interruption/resume policy-change rejection coverage.

## 0.33.0 - 2026-11-02

- Added `RoutePlanned` journal events with ranked and rejected candidates.
- Journaled objective, latency-budget and unit-cost budget value sources.
- Extended replay state and CLI replay output to recover route plans and policy sources.
- Added audit rejection when an executed route is absent from its preceding route plan.
- Preserved old journals that contain no route-plan event.
- Added runtime replay and negative route-selection audit coverage.

## 0.32.0 - 2026-11-02

- Added provider p95 latency and unit-cost limits to run budgets.
- Added matching hard-filter rejection reasons to route plans and runtime routing.
- Added CLI overrides for both limits on `run` and `route-plan`.
- Journaled effective limits through the initial task request.
- Added end-to-end coverage proving over-budget candidates are excluded with both reasons.

## 0.31.0 - 2026-11-02

- Added `route-plan` for offline provider selection inspection.
- Exposed ranked candidates and each hard-filter rejection reason.
- Added per-provider credential and endpoint readiness to route-plan output.
- Reused the runtime Router implementation, preventing planning/execution drift.
- Added end-to-end coverage proving remote and tool-capability rejections are reported without
  network calls.

## 0.30.0 - 2026-11-02

- Added quality, balanced, latency and cost route objectives.
- Kept mode, risk, privacy, tool and structured-output checks as hard filters.
- Added deterministic objective-specific ordering and tie-breaking.
- Added `--route-objective` to run and resume commands.
- Recorded the selected objective in route-selection reasons.
- Added coverage proving objective changes ranking while hard filters remain enforced.

## 0.29.0 - 2026-11-02

- Added an inert echo tool for provider tool-calling conformance.
- Added `providers --probe-tools --allow-network`.
- Validated exactly one tool call, non-empty call ID, allowed tool name and JSON arguments.
- Executed the echo observation without read, write, process or network authority.
- Required a second model call to produce a valid final answer after the observation.
- Added sanitized tool-call/observation/final-answer capability results and usage accounting.

## 0.28.0 - 2026-11-02

- Added explicit opt-in live conformance probing for OpenAI-compatible providers.
- Probes validate authentication/network handling, final structured output and usage extraction.
- Reports are sanitized to status, latency, token counts and normalized error codes.
- Network probing requires `--allow-network`.
- Added deterministic contract-validation coverage and retained offline readiness as a separate
  state.

## 0.27.0 - 2026-11-02

- Added `providers --check` readiness validation.
- Declared `api_key_env` values now fail at provider construction when missing.
- Omitted `api_key_env` remains valid for unauthenticated local endpoints such as Ollama.
- Rejected empty credential variable names and credentials on deterministic providers.
- Added child-process coverage proving readiness succeeds with a credential and never prints its
  value.

## 0.26.0 - 2026-11-02

- Added versioned release-policy files.
- Bound release mode, minimum registry version, manifest signer IDs and registry signer IDs.
- Added signer allowlist enforcement for release approvals and registry snapshot chains.
- Added machine-readable release verification reports.
- Added `configs/release-policy.example.json`.
- Kept ad hoc production flags compatible while allowing policy files to define the mode and
  minimum registry version.

## 0.25.0 - 2026-11-02

- Added explicit development and production release verification policies.
- Production requires release approval, registry key state, a signing key environment variable,
  an ordered registry snapshot chain and an external trust anchor.
- Added registry chain options to `release verify`.
- Rejected production-only options outside production mode.
- Re-checked the manifest and all artifacts after approval and chain verification.
- Verified production policy with a real CLI snapshot-chain and retired-key scenario.

## 0.24.0 - 2026-11-02

- Added `release_manifest` to the approval artifact taxonomy.
- Added expected-kind checking for approval envelopes.
- Added optional signed verification to `release verify`.
- Enforced registry key state for signed release verification.
- Verified retired keys can validate historical releases but cannot sign new releases.
- Verified revoked keys and mismatched artifact kinds fail closed.

## 0.23.0 - 2026-11-02

- Added strict, versioned external release manifests.
- Bound package version, clean Git revision, Rust toolchain and seven release artifacts.
- Required complete `SHA256SUMS` coverage for every packaged file.
- Rejected dirty releases, missing or duplicate roles, path traversal and digest drift.
- Added `release manifest` and `release verify` CLI commands.
- Integrated manifest generation into packaging and manifest equality into the release gate.
- Added archive extraction, checksum, provenance and cross-format content verification.

## 0.22.0 - 2026-11-02

- Added deterministic provenance timestamps through `SOURCE_DATE_EPOCH`.
- Recreated package output and archives before every package run.
- Generated checksums before normalizing all packaged file timestamps.
- Added sorted, ownership-neutral tar and deterministic gzip metadata.
- Added sorted zip output with normalized timestamps and no directory entries.
- Added `scripts/verify-reproducible-package.sh`.
- Verified consecutive package runs produce byte-identical Linux and Windows archives.

## 0.21.0 - 2026-11-02

- Isolated build-path reproducibility differences.
- Fixed SOURCE_DATE_EPOCH, incremental state, symbol stripping and source/target path remapping.
- Added `scripts/reproducible-build.sh`.
- Produced byte-identical Linux builds: SHA-256 `40b6b6...c484`, size `7,744,216`.
- Integrated double-build reproducibility verification into packaging.

## 0.20.0 - 2026-11-02

- Added `provenance rebuild`.
- Added fresh target-directory release builds.
- Added byte-for-byte hash and size comparison.
- Added fail-closed reproducer promotion.
- Recorded the current Linux rebuild as non-reproducible.

## 0.19.0 - 2026-11-02

- Added build provenance with commit, dirty state and tool versions.
- Added target triple, artifact size and SHA-256.
- Added `provenance create/verify`.
- Added optional signed provenance approval.
- Package output now includes Linux and Windows provenance.
- Added artifact-change rejection tests.

## 0.18.0 - 2026-11-02

- Added external trust-anchor manifests.
- Added `approval anchor`.
- Added `--anchor` verification for snapshot chains.
- Bound anchor to first snapshot digest, operator and authorized key ids.
- Required calibration snapshot chains to start at the trusted anchor.
- Added alternate-history and unauthorized-key tests.

## 0.17.0 - 2026-11-02

- Added previous-snapshot digest links and snapshot self-digests.
- Added `approval verify-chain`.
- Added contiguous chain verification.
- Rejected forks, out-of-order history and repeated versions.
- Calibration now requires a valid snapshot chain ending at current registry.
- Added fork and ordering tests.

## 0.16.0 - 2026-11-02

- Added signed approval-registry snapshots.
- Bound registry SHA-256, monotonic version, approver and admin key id.
- Added `approval snapshot/verify-snapshot`.
- Added trusted minimum registry version checks.
- Rejected registry rollback and post-snapshot mutation.
- Required snapshot validation in calibration when a registry is used.

## 0.15.0 - 2026-11-02

- Added approval key registry with active/retired/revoked states.
- Added `approval key-add`, `key-retire` and `key-revoke`.
- Retired keys verify historical approvals but cannot sign new ones.
- Revoked keys cannot verify approvals.
- Calibration requires registry-based approval verification.
- Added key-rotation and revocation tests.

## 0.14.0 - 2026-11-02

- Added detached HMAC-SHA256 approval envelopes.
- Bound approvals to artifact SHA-256, approver, purpose, timestamp and key id.
- Added `approval sign/verify`.
- Required signed gold-set approval for calibration.
- Added artifact-change and MAC-tampering tests.
- Kept unsigned artifacts development-only.

## 0.13.0 - 2026-11-02

- Added frozen gold-set manifest creation and verification.
- Added suite SHA binding, reviewer cohort, owner, redaction policy and batch assignments.
- Added manifest self-digest and drift detection.
- Required gold-set validation for calibration CLI runs.
- Added prediction rejection outside the frozen case set.
- Added gold-set manifest digest tests.

## 0.12.0 - 2026-11-02

- Added `annotate calibrate` for opt-in reliability-gated calibration analysis.
- Added ECE, Brier score, reliability bins and selective-risk points.
- Added kappa and sample-count hard gates.
- Added refusal when labels or predictions are insufficient.
- Added calibration tests for allowed and refused evidence states.

## 0.11.0 - 2026-11-02

- Added `annotate add/list/agreement`.
- Added supported/unsupported/needs-evidence labels with evidence sources and notes.
- Added duplicate case/annotator protection and hidden-control rejection.
- Added pair observed/expected agreement and kappa measurement.
- Added insufficient-data and disagreement tests.

## 0.10.0 - 2026-11-02

- Added `experience evaluate` for automated baseline-vs-shadow held-out evaluation.
- Added EvalSuite version metadata and per-case experience scopes.
- Added measured success, unsupported-certainty and duration-delta metrics.
- Added SHA-256 and case-id binding generation from the actual suite file.
- Added shadow candidate injection without activating unverified experience.
- Added positive learned-transfer and negative false-certainty tests.
- Added human-authored evaluation rubrics for conclusion terms, sources and claim statuses.
- Added rubric pass/check totals to evaluation scorecards.

## 0.9.0 - 2026-11-01

- Added `verified` experience state between review and approval.
- Added held-out verification reports with separate training/evaluation groups.
- Added gates for success improvement, unsupported-certainty regression and cost growth.
- Added `experience verify` CLI.
- Added negative tests for data leakage and increased false certainty.
- Added request-id persistence to run journals and host status journal paths.
- Added bounded journal history discovery with client request id recovery.
- Bound experience verification reports to suite version, unique case ids and SHA-256.

## 0.8.0 - 2026-11-01

- Added versioned JSONL stdio host protocol v3.
- Added stable request IDs and structured success/error responses.
- Added request-size, JSON and protocol-version validation.
- Added `serve` CLI command with configured providers.
- Added a real subprocess integration test and auditable host response.
- Added a documented host request example.
- Added `cancel` control messages keyed by request id.
- Added immediate `queue_full` and `duplicate_request` responses.
- Added structured cancellation responses and real slow-provider tests.
- Added secret-safe `providers` configuration inspection.
- Added host `status` queries with bounded recent lifecycle history.
- Added bounded journal-tail history discovery without loading observation bodies.
- Persisted host request ids in authoritative run journals.
- Added journal paths to terminal host status entries.

## 0.7.0 - 2026-11-01

- Added versioned offline evaluation suites.
- Added isolated workspace materialization for every case.
- Added expected stop reason, answer status, evidence count, claim count and audit checks.
- Added hard-gate failure accounting.
- Added machine-readable JSON reports and a human-readable CLI scorecard.
- Added `eval` CLI command and `evals/core.json`.
- Added checkpoint and tool-outcome expectations.
- Added provider, model, elapsed time and token usage to case reports.
- Verified the core suite with 4/4 cases and zero hard-gate failures.
- Produced a Windows GNU x86-64 PE release artifact; runtime execution remains unverified.

## 0.6.0 - 2026-11-01

- Added bounded workspace-wide `search_files` read-only tool.
- Added path containment, symlink, depth, file-size, file-count and result limits.
- Added generated-directory skipping.
- Added file, line, column and bounded excerpt output.
- Added hidden-control rejection for searched files.
- Added search coverage for generated-directory exclusion and path-escape rejection.

## 0.5.0 - 2026-11-01

- Added active-experience retrieval by cognitive mode, scope and expiry.
- Added context-budget-limited experience injection.
- Marked injected experience as approved but unverified policy guidance.
- Journaled candidate id, version and selection reason for every injection.
- Added CLI `--experience-dir` and `--experience-scope`.
- Added negative coverage proving draft and out-of-scope candidates are not injected.

## 0.4.0 - 2026-11-01

- Added checkpoints after every complete tool batch.
- Added resumable-prefix validation for sequence, run identity, tool pairing and terminal state.
- Added `resume` CLI and runtime API.
- Preserved canonical messages, mode, tool count, elapsed budget and provider suppression state.
- Linked each resumed journal to its original run and checkpoint sequence.
- Added runtime and real CLI interruption-equivalence tests.
- Rejected resume from completed runs.

## 0.3.0 - 2026-11-01

- Added multi-provider JSON configuration with environment-referenced API keys.
- Added ranked provider fallback for retryable failures and per-run failed-route suppression.
- Added deterministic context-budget pruning while preserving user text and recent messages.
- Added explicit claim records and evidence-to-claim reference validation.
- Upgraded journal audit to validate answers, claim references and stop-reason consistency.
- Added shared detection for bidi, zero-width, tag and unsafe control characters.
- Added config-file CLI integration coverage.
- Verified Windows GNU cross-target compilation after the new modules.

## 0.2.0 - 2026-11-01

- Added L2 experience candidates for memory, skill and route-policy lessons.
- Bound candidates to audited run journals and existing evidence event sequences.
- Added review, approval, rejection, activation, degradation and rollback transitions.
- Added invisible Unicode and empty-text rejection for experience writes.
- Added CLI commands for the complete candidate lifecycle.
- Added unit coverage and a real CLI end-to-end experience lifecycle test.
- Verified Windows GNU cross-target compilation with `cargo check`.

## 0.1.0 - 2026-11-01

- Created the Rust/Tokio agent runtime.
- Added structured task, answer, model, tool and journal contracts.
- Added deterministic provider and OpenAI-compatible adapter.
- Added capability-based static provider routing.
- Added read-only, root-confined file evidence tool.
- Added JSONL replay and audit commands.
- Added unit, CLI end-to-end and loopback provider tests.
- Added architecture, ADR and verification documentation.
