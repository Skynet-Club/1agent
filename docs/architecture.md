# Architecture

Status: accepted for V0

## Goals

The runtime must be cross-platform, provider-neutral, recoverable, auditable and safe by default. The domain core owns cognition state, evidence, policy, stop reasons and the run journal contract. Hosts and providers are adapters.

## Layers

```text
CLI host
  -> Ratatui/Crossterm interaction layer
    -> input editing, mouse selection, clipboard and command drawer
    -> structured transcript and sticky user-instruction bar
  -> application service
    -> agent runtime
      -> model router -> provider adapter
      -> tool pipeline -> read-only tools
      -> run journal -> JSONL store
```

The terminal UI is a presentation adapter. It calls the same `AgentRuntime`, answer contract,
provider router and journal as the one-shot CLI; it does not own or mutate domain state directly.

The command surface is host-aware. A `HostBinding` distinguishes standalone 1A,
Workshop Master and Workshop Worker. Core commands are available everywhere; Master
production-control commands are visible and executable only in the Workshop Master
binding. Command visibility is a UX rule, not an authorization boundary, so Master
handlers must revalidate state and adapter authority. The drawer groups the empty
query by category and puts recent commands first; an open command panel owns the
keyboard, and destructive clear/dispatch/control actions require a second Enter.
These are interaction guards, not a replacement for authority checks. A bare TUI
launch starts a new session and the exit banner identifies the session and its
continue command; `Ctrl+Q` requires Enter confirmation; Direct Answer mode keeps
only the conclusion in the transcript; and the running line reports elapsed time,
input/output tokens and provider cache hit rate when reported.

The TUI session store uses version 2 metadata with a version 1 compatibility path and a
separate append-only `events.jsonl` for host binding, command decisions, task submission
and session closure. These events are correlated with, but do not replace, run journals
or Master journals.

First-principles commands use two paths. `/frame`, `/challenge`, `/decide` and
`/postmortem` create model runs with an explicit cognitive mode. `/evidence`, `/audit`
and `/report` are deterministic journal views and never upgrade a model statement into
verified evidence.

Workshop Master task definition is session-bound before it enters the execution state
machine. `/mission` records the goal and workspace, `/triage` uses deterministic routing,
and `/plan` uses the existing `ModelPlanGenerator` plus deterministic validation. Plan
files remain unapproved. `/approve` binds user approval to the exact plan SHA-256; phase 3
must convert that explicit approval into a durable MasterState/lease before dispatch.

Phase 3 performs that conversion. `/agents` merges configured profiles with Workshop
Registry metadata, `/assign` applies the existing deterministic assignment filters, and
`/dispatch` initializes or reloads MasterState, materializes approval, creates leases and
calls the Workshop adapter. Provider selection remains subordinate to router hard filters;
the TUI can pin or change route objective but cannot bypass capability, privacy or risk
constraints.

Phase 4 adds supervision over that durable state. `/board` renders replay-validated Master
state, `/watch` consumes bounded Session progress markers, `/attention` isolates operator
work and `/respond` routes a human message back to the exact Session. `/control` records
explicit pause, resume, cancel, takeover, retry and replan decisions after validating
state and bounded counters. `/timeline` merges bounded Master and Workshop evidence without
treating the Workshop scheduler as a second Master state store.

Phase 5 closes the acceptance loop. `/artifacts` re-inspects current files, `/verify`
requires exact independent evidence and a verifier distinct from the execution Agent,
and Master `/evidence`, `/audit` and `/report` project the same durable record into
claim, production and audit views. Execution success is never promoted to acceptance.

Phase 6 adds local TUI controls for sessions, workspaces, context, presets, exports,
clipboard, and bounded run preferences. Runtime context pruning and explicit local
compaction reduce repeated context while reporting character headroom rather than
inventing exact provider-token counts. Session metadata carries preferences, preferred
workspace, mentions, archive state and an invalidatable context summary.

Usage accounting is rebuilt from journaled provider responses rather than mutable UI state.
Provider-reported cost is preserved; configured route prices fill missing cost. Context
pruning triggers automatically at a bounded percentage before the hard character limit.

## V0 decisions

- Rust 2024 and Tokio for a portable single-binary core with explicit cancellation and timeouts.
- JSONL append-only run journal as the first durable fact store.
- Canonical internal message, tool-call, observation and answer types.
- OpenAI-compatible HTTP adapter is only a provider adapter, not the internal protocol.
- Deterministic fake provider is required for offline tests and evaluation.
- Read-only file tool is the first real capability.
- The terminal UI remains a presentation adapter; no database, vector store, agent framework or
  online self-modification is part of the runtime.

## Implemented topology

```text
first-principles-agent CLI
  -> AgentRuntime
    -> deterministic mode inference
    -> static capability router
    -> DeterministicProvider or OpenAiCompatibleProvider
    -> read-only file/search tools
    -> context budget and durable checkpoints
    -> Journal / JSONL
    -> replay, semantic audit and offline evaluation
    -> versioned scorecards -> content digest -> optional approval envelope
    -> experience lifecycle and held-out promotion gates
    -> stdio host protocol

release pipeline
  -> two isolated builds per target
  -> normalized package tree
  -> deterministic tar.gz and zip
  -> build provenance and complete SHA256SUMS
  -> external release manifest
  -> optional approval and production snapshot-chain verification
```

The V0 runtime runs in the CLI process. Application boundaries are defined so that an
independent local service can replace the in-process host later without changing domain
events or provider/tool contracts.

## Product runtime home

`1agent init` creates one private home (`$ONEAGENT_HOME` or `$HOME/.1a`) marked by
`home.json` (layout version 2 keeps key material under `keys/`). `doctor`
reports marker/version, permissions, layout, master-key and writability checks
and verifies any task-graph/memory/skill journal hash chain;
`export`/`restore` use a hash-manifest bundle that excludes key
and credential files; `upgrade` repairs the current layout and rejects unknown
versions; `uninstall` requires `--purge --yes` and refuses unsafe paths. Once a
home is initialized, CLI path defaults (journals, task graph, memory payloads and
key, master, experience, annotations) resolve inside it; explicit flags still
override. Uninitialized worktrees keep historical relative paths. `key escrow`
encrypts the master key and KEK ledger under a separate 0600 recovery key;
`key restore` verifies and restores them without overwriting existing material.
The task-graph, memory, skill, goal and action stores take a sibling `flock`
around tail repair/replay and append, and reject a stale writer by byte length.
Memory payload/key operations take `<memory.key>.lock` and reload the keyring
under it; delegation/evolution snapshots use a lock plus SHA-256 version check.
Long-lived services reload after a conflict and surface a retryable error. There
is no cross-process index or automatic merge. Provider health alerts can be
posted to an HTTPS webhook from `run`/`serve`. See ADR-0024 for the home/export
boundary, ADR-0025 for key escrow, ADR-0026 for cross-process locks and ADR-0027
for the alert webhook.

## Master-Agent V1

```text
natural-language goal
  -> deterministic triage
  -> model PlanProposal (bounded, unapproved)
  -> deterministic DAG / budget / permission / Artifact validation
  -> capability-filtered AssignmentProposal
  -> human approval
  -> persisted TaskLease + idempotency key
  -> ExecutionPort
    -> Skynet Workshop Adapter
      -> agent-run / persistent Session
  -> ExecutionReceipt
  -> independent verifier
  -> verified dependency unlock
  -> bounded retry or affected-subgraph replan
  -> Master Journal replay / audit
```

Master coordination remains in 1A. Skynet Workshop retains authority over Session, Pane,
Job, process, capacity and execution controls. 1A never duplicates the Workshop scheduler
or treats a Pane as a Work Item identity.

The Master store uses a versioned JSON snapshot plus an append-only JSONL event journal.
Every event contains the post-transition state; load prefers replaying the journal and
`audit` validates sequence, identity, bounded counters and status transitions. Planner
requests and responses are separately journaled before a plan enters the Master store.

Default safety limits are eight Work Items, three parallel tasks, one automatic retry per
task and one automatic replan per run. A plan must be approved before dispatch. A
nonterminal lease cannot be silently replaced even after expiry; explicit cancel/takeover
is required. Verification fails closed unless execution exited zero, every acceptance
criterion has independent passing evidence and every expected Artifact exists under the
workspace with optional SHA-256 matching.

The standalone `verification::verify` contract (ADR-0037) makes the same
distinction reusable outside Master: a distinct verifier, exactly one result
per acceptance/hidden criterion, non-empty evidence for a pass, and one
artifact check per expectation. Outcomes are `verified`, `rejected` or
`hacked` (public pass, hidden fail); `summarize` exposes
`hacked_resolved_bps` so a high public pass rate cannot hide reward hacking.

## Implemented contracts

- `TaskRequest`: objective, deliverable, known facts, constraints, unknowns, decision and
  success criteria, permissions and budgets.
- `AgentAnswer`: result status, conclusion, evidence, assumptions, constraints, unknowns,
  flip conditions and action receipts.
- `JournalEvent`: protocol version, run id, contiguous sequence, timestamp and typed event.
- `ModelRequest` and `ModelResponse`: canonical provider-neutral messages and tool calls.
- Malformed final model text is retained as `InvalidFinal` so the runtime can repair it without
  losing the original response or exhausting provider failures before the repair attempt.
- `ToolObservation`: structured status, content, error code and metadata.
- `EvalReport`: suite outcome counts, decision-completeness policy version/source,
  canonical content digest and per-case results.
- `EvalReportSummary`: case-content-free status view with schema version, integrity,
  content-binding, signature-presence and authenticity states.
- `ApprovalEnvelope`: HMAC-authenticated binding to exact report bytes, constrained by
  approval key registry state.

## Memory key hierarchy

- Schema-2 payloads use a random per-scope KEK in `<key-file>-keks.json`, wrapped by a
  master-derived AEAD key; payloads record `kek_version`.
- `rotate_scope_key` rewraps live payloads, then retires older versions. A pre-rotation
  backup referencing a retired version is unrecoverable even with the master key.
- Schema-1 payloads remain readable via the legacy derivation; rotation migrates them,
  and master-key rotation (`memory rotate-master-key`) adds a new master, rewraps the KEK
  ledger, then removes old masters. It refuses to run while schema-1 payloads exist.

## Agent intent, attention and evolution

- `GoalGraph`/`GoalStore`: append-only, hash-chained Goal/Commitment events with JSONL
  replay; commitments can reference a Ticket but do not grant execution authority.
- `attention_candidates`: deterministic, read-only projections over due, overdue and
  missed commitments plus due goals; fixed urgency ordering and a caller budget.
- `DelegationRegistry`: observe/suggest/draft/ask/act_and_report profiles intersected with
  Authority, live approval and reversibility; `act_silently` is not representable.
- `build_context_graph`: rebuildable Goal/Commitment/Ticket/Thread/Memory view with
  explicit edges; hard-forgotten memory is absent and superseded memory is marked.
- `EvolutionRegistry`: human-gated proposals for skills/routines/strategy only, with
  hash-chained receipts; authority/policy/persona changes are not representable.
- `UserModel` is a derived view over scoped memory records (ADR-0035). Besides
  preferences and boundaries it exposes `autonomy.<domain>` (`observe` /
  `suggest` / `act` / `auto`, default `suggest`), `voice` / `voice_mode`
  (`operator` / `advisor` / `companion`, default `operator`) and
  `consent:<kind>=allow|deny[@<retention_ms>]` (default allow). Each entry keeps
  its source memory id; forgotten or superseded records are ignored. Autonomy
  and consent are preferences, not grants: Authority/Policy/Approval still
  decide execution. `1agent user-model` prints the view as JSON or text.
- `autonomy`: `decide(level, risk, reversible, explicit_approval)` maps a
  per-domain preference into `observe` / `suggest` / `act` /
  `require_approval`. `observe` and `suggest` never act; `act` allows only
  low-risk reversible actions; `auto` allows low/medium risk; high risk and
  irreversible actions require approval unless explicitly approved.
  `decide_proactive` adds quiet hours (`QuietHours`, cross-midnight windows) and
  returns `Defer` for proactive act/require_approval inside the window, with
  minutes until it ends; user-initiated work and explicit approval are never
  deferred. This is a preference policy; Authority/Policy/ExecutionGate still
  enforce execution.
- `alarm_budget`: given the delivery window counts, an ordinary alert is
  delivered while the budget and false-alarm ceiling hold, otherwise moved to
  a digest. Alerts at or above `critical_urgency` bypass both. This module
  does not send, persist or acknowledge alerts.
- `escalation`: an ordered ladder of `(after_ms, action)`; `plan` selects the
  highest rung whose threshold the item has reached, returns no level before
  the first threshold or after resolution, and requires strictly increasing
  thresholds. It computes a level; delivery is separate.
- `org_audit`: append-only organization index. Each entry carries seq,
  actor/action/scope/decision, the previous hash and its own SHA-256 hash over
  a canonical field body. `append` writes and syncs one JSON line; `verify`
  detects edited fields, reordering and broken links. It is a local index, not
  a remote transparency log, and it does not authenticate the actor.
- `mcp_auth`: policy over already-signature-verified MCP OAuth claims. It
  enforces exact audience, expiry/not-before with clock skew, a maximum TTL and
  an exact required scope. Signature verification and token acquisition are
  the caller's; this module never stores or prints the token.
- `handoff`: grounded team handoff notes. `create` requires distinct from/to,
  a non-empty summary and at least one evidence reference; only the addressee
  may `accept`, and a terminal handoff cannot transition again. Snapshots are
  appended and synced to JSONL; the latest per id wins.
- `agent_card`: `AgentCard` states schema version, name, version, platform,
  capability statuses and limits. Platform-constrained capabilities come from
  the platform registry; core capabilities reflect contract tests;
  `cross_platform` is untested, team collaboration is degraded (policy, handoff
  notes and local audit only) and marketplace is unsupported.
  `1agent agent-card` prints it and `GET /v0/agent-card` serves the same
  payload on the public read-only HTTP Surface. `--generate-key` creates an
  Ed25519 key pair, `--sign-key` signs the canonical payload (the card without
  the signature field), and `--verify PATH --trusted-key HEX` rejects tampered,
  wrong-key and unsigned cards. This is a local trust statement, not A2A
  registry discovery or a signed registry entry. `card_trust` keeps a local
  JSON store of trusted/revoked Ed25519 public keys; verification requires the
  card's embedded key to be trusted and unrevoked before delegating to
  `agent_card::verify_card`. The store is a trust anchor and must be protected
  like a keyring; it does not discover or fetch keys remotely.
- `onboard`: `1agent onboard --voice ... --autonomy DOMAIN=LEVEL --boundary
  TEXT` writes ordinary memory records (`autonomy.<domain>=...`,
  `voice_mode=...`, `boundary: ...`) and prints the resulting `UserModel`.
  Invalid values fail before writing. Optional `--goal/--commitment` write a
  first goal and commitment to `GoalStore`; `inbox sync --goal-journal` can
  project an overdue commitment. `--import-file` reads plain text one memory
  per line, skipping `#` comments and blanks, bounded to 256 KiB, 200 usable
  lines and 2000 chars per line; the file must be a regular non-symlink file
  and is not redacted. This is v0 profile setup, not the full
  interview/calendar/email import flow, and memory/goal writes are not atomic.
- `repair`: `plan_repair` classifies an applied side effect as `none`,
  `compensate` or `manual`; an applied reversible action without a recorded
  compensation fails closed, and an irreversible action always requires a
  human. `build_away_digest` projects the side-effect ledger into applied,
  compensated, failed, pending, `needs_user` and receipts; `1agent
  away-digest` exposes the view. This slice plans compensation; executing it
  still requires Authority/Policy/Approval.
- `value_report`: `ValueReport` counts runs by stop reason, tokens/cost, tool
  calls/failures, alerts and pending decisions over a bounded window, plus
  cost per completed outcome and the open inbox queue size. When the caller
  supplies an independently verified outcome count, it also reports cost per
  verified outcome. `1agent value-report` exposes it. v0 does not invent a
  time-saved estimate.
- Voice modes: `voice::with_voice` appends an `operator` / `advisor` /
  `companion` block to the base system prompt. `AgentRuntime` defaults to
  `operator`; `with_voice_mode` selects another mode and `1agent run
  --voice-mode` exposes it. The block changes how the answer speaks, not the
  evidence, authority or action rules.
- `inbox`: an append-only JSONL queue of `notify` / `question` / `review`
  items. Each mutation writes a full snapshot; replay keeps the latest state.
  `open_sorted` returns open or due-snoozed items ordered review → question →
  notify, then due time. Terminal statuses cannot transition; duplicate ids,
  empty fields and invalid snoozes fail closed. `1agent inbox
  add|list|resolve|snooze` is the local operator surface. `project_attention`
  maps overdue/missed commitments to `review` and due items to `notify`;
  `project_alerts` maps failed/error alerts to `review`; `project_handoffs`
  maps open handoffs addressed to the recipient to `review` with
  `handoff:<id>`. `inbox sync` merges projections with dedupe by `item_id`; it
  is dry-run by default and `--apply` writes. Resolving a `handoff:<id>` item
  as `approved` requires `--handoff-store` and `--actor`; the handoff is
  accepted first, then the inbox item is closed. Other terminal statuses do not
  accept the handoff. Sync never updates an existing item's status. The TUI exposes a
  `/inbox` transcript command for the default store, with inline
  `resolve`/`snooze` actions parsed by the same `parse_action`/`apply_action`
  contract as the CLI.

## Security behavior

- Surface HTTP derives its `AuthContext` from the server configuration (bearer principal),
  not from the request body. A caller cannot claim another actor or extra scopes.
- `ContextSummary` requires `context:read` + `memory:read` and filters memory by `bot_id`.
- `SurfaceService::open` persists security actions to a sibling
  `<graph-stem>-action-log.jsonl`; the log is verified on open and tampering fails closed.
- Read access is disabled without configured roots.
- Canonicalized file paths must remain below a configured read root.
- Write authority is true by default for local CLI/TUI; `--read-only` disables it.
  Surface/API callers must opt in. Write tools are filtered from the provider
  tool list and denied by the registry while it is off.
- Terminal tools are high-risk host adapters backed by Skynet Workshop sessions.
  They require write authority, cap observations at 64 KiB and strip ANSI/control
  characters before journaling; command output is still journaled, so secrets
  must not be printed into a terminal.
- `read_pdf` extracts local PDF text through the system `pdftotext`: read-root
  containment, symlink rejection, a character cap, control-character rejection
  and page form-feed normalization. It does not perform OCR or understand images.
- `fetch_url` is read-only but network-gated: HTTPS only, public addresses only,
  credentials in URLs rejected, redirects revalidated and capped at 5 hops, text
  content only and a 2 MiB response cap.
- `web_search` uses the same fetch guard and Bing's RSS endpoint as a
  best-effort, no-key provider. Results are bounded and deduplicated; provider
  rate limits are reported as tool failures, not fabricated results.
- MCP tools are discovered from stdio or streamable-HTTP servers configured
  with `--mcp-config`. Exactly one of `command` or `url` is required. HTTP
  requires HTTPS except for loopback, rejects credentials in the URL, carries
  `Mcp-Session-Id`, accepts JSON or `text/event-stream` responses, and reads a
  bearer token only from the environment variable named by
  `bearer_token_env`. A tool is write-gated unless the server declares
  `readOnlyHint: true`; `destructiveHint: true` raises its risk. The MCP server
  validates its own input schema; 1Agent owns process, permission, timeout and
  journal bounds.
- `spawn_agent` starts a bounded child `1agent run` with its own journal,
  workspace and budget. Depth is capped at 2, write/network permissions are
  inherited, and the child is killed with the parent process.
- `schedule` persists interval jobs under the runtime home and `daemon` runs
  due jobs as isolated child processes. A pid lock prevents duplicate daemons;
  an overdue persisted job is caught up after restart, and `schedule health`
  reports `ok`/`overdue`/`failing`. Failed or timed-out runs can append a
  `schedule_run_failed` alert and deliver it to a webhook. With `--heartbeat`,
  each daemon loop writes an atomic `{pid, updated_at_ms}` file; a separate
  `1agent watchdog --heartbeat PATH --max-age-ms N` process exits non-zero when
  the heartbeat is missing, stale or invalid. Escalation and cross-restart
  deduplication are not yet wired into the daemon.
- A job may bind an active skill with `--routine-skill NAME@VERSION`; the child
  run composes that version's body into its task. This reuses the approved
  skill without granting permissions, and rejects inactive or mismatched
  versions.
- Skill tools expose the existing registry to the model: list/eval are read-only,
  register/activate/rollback are write-gated and journaled through `SkillService`.
- `skill export` writes immutable manifests to a versioned JSON bundle; `skill
  import` requires a matching SHA-256, rechecks every manifest's body hash and
  skips versions that already exist. It is a distribution primitive, not a
  marketplace: there is no remote index or signature yet.
- `skill scan --path SKILL.md` parses and validates a skill without registering
  it. It runs the registry scan: control characters, private-key markers,
  secret-like tokens and instruction-override markers (`ignore previous`,
  `disregard previous instructions`, `reveal your system prompt`,
  `print your instructions`, `jailbreak`). This is a preflight heuristic, not
  a malware sandbox.
- `skill verify --bundle PATH --sha256 HEX [--public-key PATH]` calls the same
  `verify_bundle` used by import: size, SHA-256, schema, signature (signed
  bundles require a trusted key; unsigned with a key is rejected), duplicate
  names and manifest hashes, and the registry scan. It does not import.
- `cua`: computer-use provenance and branch policy. An action's arguments are
  tagged `plan`, `trusted_observation` or `untrusted_observation`; a high-risk
  action with an untrusted argument is denied, other untrusted arguments
  require approval, and irreversible actions above low risk require approval.
  The planner must not see untrusted content, and a branch target must be in
  the pre-approved plan (branch-steering guard). This is policy only; no
  browser execution.
- `team`: role/action/scope policy. Owner allows all; approver allows
  read+approve; member allows read+write; viewer allows read. Empty scopes mean
  all; a non-empty list must contain the requested scope. Unknown actors,
  disallowed actions and ungranted scopes are denied. `1agent team-check`
  exposes it. This is policy, not an identity provider.
- `idempotency`: `IdempotencyStore` persists key → fingerprint → response as
  append-only JSON lines and `sync_data`s each record before acknowledging.
  Re-opening the store replays the original response for the same key and
  fingerprint; a different fingerprint is a conflict. A final line without a
  newline is an interrupted append and is ignored. This is the cross-restart
  dedupe primitive; wiring it into every external write is separate.
- `chat`: `to_task_request` maps a `ChatMessage` to a `TaskRequest`;
  `format_reply` returns the conclusion, one trust line (`证据 N · 未知 M`),
  `needs_approval` and `inbox_open`, without schema field names. This is the
  channel-neutral contract; Slack/Telegram/WeChat transports are separate.
- `otel`: `spans_from_events` projects the journal into deterministic
  OTel GenAI-style spans: one `invoke_agent` root, one `chat` span per model
  response with provider/model/token/cost attributes, and one `execute_tool`
  span per tool observation. `1agent otel` prints the JSON spans. This is a
  projection, not an OTel SDK/wire exporter.
- `budget`: optional cost/token/tool-call limits; exceeding any limit is a
  failure, 80% is a warning, unset limits are unbounded.
  `cost_per_verified_outcome` returns `None` when nothing verified.
  `1agent budget-check` exposes it and exits non-zero on an exceeded budget.
  `RunBudget` also carries `max_cost_micros`/`max_tokens`; the runtime
  accumulates provider-reported usage and stops with
  `StopReason::BudgetExhausted` after a response exceeds either limit.
  `1agent run --max-cost-micros/--max-tokens` sets them.
- `cascade`: `simulate(root, edges, contained)` walks a directed failure graph;
  a contained node is affected but stops propagation. The report exposes
  `containment_rate_bps = contained / affected`, `cascade_depth` and
  `blast_radius`; `evaluate` gates them. `1agent cascade-check` exposes it.
  This is a deterministic simulation, not a live multi-agent run.
- `tool_search`: `search(query, specs, limit)` ranks tools by exact/prefix/name
  token, description token and parameter-name token, deterministically.
  `select` adds the always-on tools (`read_file`, `write_file`, `edit_file`,
  `terminal_exec`) to the top hits. `1agent tools --query` exposes the ranking;
  `1agent run --tool-query` sets the advertised list, which is journaled in
  `ModelRequested`. With no query, the full permission-filtered list is used.
- Delegation tools expose profile registration/revocation through the existing
  snapshot store; they do not grant authority by themselves.
- Surface goal, commitment, skill and evolution-ledger lists accept
  `limit`/`cursor`; cursors are ids (skill cursors use `name@version`) and the
  per-actor quota still applies.
- The context graph can be cached by a deterministic fingerprint of goals,
  tasks and memory references; stale fingerprints miss and rebuild, and the
  graph remains a projection rather than a second source of truth.
- Execute-mode requests stop at `needs_authorization` without calling a provider.
- Provider network access requires explicit `--allow-network`.
- API keys are read from an environment variable or hidden CLI option and are not journaled.
- Non-loopback provider credentials require HTTPS; plaintext HTTP is limited to loopback development.
- Release verification fails closed on dirty source, missing artifact roles, digest drift,
  incomplete checksums, invalid approval kind, revoked keys or unanchored registry history.
- Report inspection always validates policy compatibility and internal aggregate consistency.
- Content-bound reports detect payload mutation by recomputing `content_sha256`.
- Authenticated report inspection requires an `evaluation_report` approval envelope and
  Active/Retired, non-revoked registry key; unauthenticated status is never upgraded implicitly.

## V0 limitations

- Proving real-model quality without configured providers.
- Structured patch/edit tools beyond whole-file replacement (`write_file`) and
  exact text replacement (`edit_file`).
- Web search is best-effort Bing RSS; authenticated web content, computer-use
  clicks/typing/screenshots, and MCP OAuth issuer discovery/token exchange are
  absent.
- Voice/image understanding and OCR; `read_pdf` only extracts embedded PDF text.
- Page-range selection for PDFs; `max_chars` only bounds returned text.
- Terminal commands require a running Skynet Workshop session daemon; there is
  no native PTY backend yet.
- Streaming model output.
- Raw-mode terminal behavior on Windows.
- Arbitrary multi-agent organization beyond the bounded eight-item V1 coordinator.
- Multi-host or remote Workshop control.
- Modifying Skynet Terminal.
- Asymmetric release signatures; current approval is HMAC-based.
- Asymmetric report signatures; report authentication is currently HMAC-based.
- Cross-machine hermetic toolchain reconstruction.
- macOS verification and Windows runtime execution.

## Evaluation trust boundary

The report trust chain separates four claims:

1. **Integrity checked**: policy metadata, arithmetic, aggregates and optional digest passed.
2. **Content binding**: the report contains a canonical digest of its own payload.
3. **Signature present**: an external approval envelope was supplied and verified.
4. **Authenticity verified**: signature verification succeeded for the configured key registry.

These claims do not establish evaluation truth or model reasoning quality. Unsigned reports
remain inspectable for diagnostic use, while hosts may require content binding or authenticity.
See `docs/adr/0004-evaluation-report-trust.md`.

## 1Agent A1/A2 contracts

The A1/A2 slices add domain modules that are independent of hosts, providers and UI:

- `task_graph`: hash-chained, append-only events for Trigger, Thread, Eval, Decision,
  Ticket, Mandate, Approval, Assignment, Report and SideEffect. Replay verifies the
  chain; write operations persist through `TaskGraphService`.
- `memory`: scoped records with source, taint, TTL, soft/hard forget, plus a metadata
  journal and AES-256-GCM payloads (per-record DEK, per-scope KEK). The journal never
  contains body text. Backup export excludes the key and, under ADR-0021, hard forget
  does not claim coverage of existing backups.
- `skill`: SKILL.md registry with versions, scanning, activation/rollback and a
  hash-chained journal. Replay rescans manifests and verifies body hashes.

Read-only inspection uses `TaskGraphService::open_read_only`; it never repairs or rewrites
the log. Writers assume a single process and enforce private file modes.

## Cross-platform rules

- Domain and application code must not branch on `cfg(target_os)`.
- Platform capabilities belong in adapters with explicit `verified`, `degraded`, `unsupported` and `untested` status.
- `platform::report()` and `1agent platform [--json]` expose that status map
  (ADR-0036). Linux currently reports verified core paths, degraded adapters and
  unsupported computer-use; macOS/Windows/other report every capability as
  `untested` until the P-Gate exercises them.
- File access is constrained to canonicalized read roots.
- Cancellation must propagate through provider calls, tools and cleanup.
- Structured errors are part of the contract; provider-specific error text is not.
- Provider responses that cannot be interpreted as `AgentAnswer` are repaired once within the
  run budget, then fully parsed and domain-validated again before becoming an answer.
