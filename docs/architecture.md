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

## Security behavior

- Surface HTTP derives its `AuthContext` from the server configuration (bearer principal),
  not from the request body. A caller cannot claim another actor or extra scopes.
- `ContextSummary` requires `context:read` + `memory:read` and filters memory by `bot_id`.
- `SurfaceService::open` persists security actions to a sibling
  `<graph-stem>-action-log.jsonl`; the log is verified on open and tampering fails closed.
- Read access is disabled without configured roots.
- Canonicalized file paths must remain below a configured read root.
- Write authority is false by default.
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
- Calling Linux commands or writing files.
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
- File access is constrained to canonicalized read roots.
- Cancellation must propagate through provider calls, tools and cleanup.
- Structured errors are part of the contract; provider-specific error text is not.
- Provider responses that cannot be interpreted as `AgentAnswer` are repaired once within the
  run budget, then fully parsed and domain-validated again before becoming an answer.
