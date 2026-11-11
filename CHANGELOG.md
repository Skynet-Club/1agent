# Changelog

## Unreleased — 0.97 Super-Partner groundwork

- `UserModel` 2.0 (ADR-0035): derived view now carries per-domain autonomy
  (`observe`/`suggest`/`act`/`auto`), voice mode
  (`operator`/`advisor`/`companion`) and memory consent
  (`consent:<kind>=allow|deny[@<retention_ms>]`). Defaults are safe: `suggest`,
  `operator`, allow; forgotten/superseded records never grant more.
- New `1agent user-model --journal ... --payloads ... --key ... --bot-id ...`
  command prints the relationship view as JSON or human-readable text.
- Tests: seven domain tests plus two CLI e2e tests
  (`cargo test user_model --lib`, `cargo test --test user_model_cli`).
- Platform capability registry (ADR-0036): `1agent platform [--json]` reports
  `verified` / `degraded` / `unsupported` / `untested` per capability. Linux
  core paths are verified; macOS/Windows/other are all `untested` and are not
  claimed as supported.
- Tests: three platform tests plus two CLI e2e tests
  (`cargo test platform --lib`, `cargo test --test platform_cli`).
- Independent verification contract (ADR-0037): `verification::verify` requires
  a distinct verifier, exactly one result per acceptance/hidden criterion,
  non-empty evidence for a pass and one artifact check per expectation. A
  public pass with a hidden failure is `hacked`; `summarize` reports
  `hacked_resolved_bps`.
- Tests: seven verification unit tests plus the `hack_verifiable` contract
  suite (`cargo test verification --lib`, `cargo test --test hack_verifiable`).
- Trust & Repair (ADR-0038): `repair::plan_repair` classifies applied side
  effects as `none`, `compensate` or `manual`; an applied reversible action
  without a recorded compensation fails closed. `build_away_digest` and
  `1agent away-digest` show applied/compensated/failed/pending/needs_user plus
  per-action receipts. This slice plans compensation, not executes it.
- Tests: six repair unit tests plus two `away_digest_cli` e2e tests
  (`cargo test repair --lib`, `cargo test --test away_digest_cli`).
- Value Report v0 (ADR-0039): `value_report::build_value_report` counts runs by
  stop reason, tokens/cost, tool calls/failures, alerts, open inbox items and
  pending decisions over a bounded window, plus cost per completed outcome.
  `1agent value-report` exposes it. It does not invent a time-saved estimate.
- Tests: two value-report unit tests plus three `value_report_cli` e2e tests
  (`cargo test value_report --lib`, `cargo test --test value_report_cli`).
- Partner voice modes (ADR-0040): `operator` (default), `advisor` and
  `companion` blocks are appended to the base system prompt.
  `AgentRuntime::with_voice_mode` and `1agent run --voice-mode` select one.
  The block changes how the answer speaks, not the evidence/authority/action
  rules; companion is warm but does not flatter or role-play.
- Tests: three voice unit tests plus two `voice_mode_cli` e2e tests
  (`cargo test voice --lib`, `cargo test --test voice_mode_cli`).
- Agent Inbox domain/store (ADR-0041): append-only JSONL snapshots for
  `notify` / `question` / `review` items, snooze and terminal statuses, with
  duplicate-id, empty-field, invalid-transition and invalid-snooze failures
  closed. `open_sorted` orders review → question → notify, then due time.
- Tests: five inbox unit tests (`cargo test inbox --lib`).
- Agent Inbox CLI and projections (ADR-0042): `1agent inbox
  add|list|resolve|snooze`; `project_attention` maps overdue/missed
  commitments to review and due items to notify; `project_alerts` maps
  failed/error alerts to review. Projections are pure; `inbox sync` is next.
- Tests: two more inbox unit tests and two `inbox_cli` e2e tests
  (`cargo test inbox --lib`, `cargo test --test inbox_cli`).
- Inbox sync (ADR-0043): `InboxStore::sync` merges projected items with dedupe
  by `item_id`; `1agent inbox sync --goal-journal/--alert-log` is dry-run by
  default and writes only with `--apply`. Sync never overwrites an existing
  item's status.
- Tests: one sync unit test and two `inbox_sync_cli` e2e tests
  (alert log and goal journal; `cargo test inbox --lib`,
  `cargo test --test inbox_sync_cli`).
- Autonomy dial policy (ADR-0044): `autonomy::decide` maps
  observe/suggest/act/auto plus risk, reversibility and explicit approval to
  observe/suggest/act/require_approval. `observe`/`suggest` never act; `act`
  allows only low-risk reversible actions; `auto` allows low/medium risk; high
  risk and irreversible actions require approval unless explicitly approved.
  `1agent autonomy` exposes the policy. A preference is not an authorization
  grant.
- Tests: five autonomy unit tests plus two `autonomy_cli` e2e tests
  (`cargo test autonomy --lib`, `cargo test --test autonomy_cli`).
- Onboarding v0 (ADR-0045): `1agent onboard --voice ... --autonomy
  DOMAIN=LEVEL --boundary TEXT` writes ordinary memory records and prints the
  resulting `UserModel`. Invalid levels, voices, domains and boundaries fail
  closed. This is scriptable profile setup, not the full interview/context
  import.
- Tests: two `onboard_cli` e2e tests (`cargo test --test onboard_cli`).
- Onboarding v0.1: optional `--goal/--goal-journal/--goal-due-ms` and
  `--commitment/--commitment-due-ms` seed a first goal and commitment in
  `GoalStore`; `inbox sync --goal-journal` can project an overdue commitment.
  Memory and goal writes are not atomic.
- Tests: two more `onboard_cli` e2e tests for goal seeding and
  commitment-without-goal rejection (`cargo test --test onboard_cli`).
- Onboarding v0.2: `--interactive` prompts on stderr for voice, first goal,
  first commitment and boundary and reads one line per prompt from stdin.
  Blank answers keep flags; invalid values still fail closed. It is a
  scriptable interview, not a free-form conversation or context import.
- Tests: one interactive `onboard_cli` e2e test with piped stdin
  (`cargo test --test onboard_cli`).
- TUI `/inbox` view and inline actions (ADR-0046): lists open or due-snoozed
  items with kind, status, title and `why_now`; supports `/inbox resolve <id>
  <approved|rejected|answered|dismissed> [note]` and `/inbox snooze <id>
  <until_ms>` with the same parser/applier as the CLI.
- Tests: inline action parsing plus `/inbox` registration and non-panel
  behavior (`cargo test inbox --lib`, `cargo test tui_commands --lib`).
- Agent Card v0 (ADR-0047): `1agent agent-card [--json]` emits a machine-readable
  card with schema/version/platform, capability statuses and limits. Platform
  constraints come from the platform registry; `cross_platform` is untested and
  team/marketplace are unsupported. The card is local and unsigned, not yet an
  A2A HTTP service.
- Tests: three agent-card unit tests plus two `agent_card_cli` e2e tests
  (`cargo test agent_card --lib`, `cargo test --test agent_card_cli`).
- Agent Card over HTTP (ADR-0051): `GET /v0/agent-card` serves the same
  unsigned card as a public read-only route; no bearer or body is required.
- Tests: HTTP surface route returns 200 with the card name and non-empty
  capabilities (`cargo test http_surface --lib`).
- Skill scan (ADR-0048): `1agent skill scan --path SKILL.md [--json]` validates
  frontmatter and runs the registry scan without registering or executing the
  skill. Adds instruction-override markers (`disregard previous instructions`,
  `reveal your system prompt`, `print your instructions`, `jailbreak`).
- Tests: two `skill_scan_cli` e2e tests for a safe skill, prompt injection and
  malformed frontmatter (`cargo test --test skill_scan_cli`).
- Skill bundle verify (ADR-0052): `verify_bundle` is shared by import and the
  new `1agent skill verify --bundle PATH --sha256 HEX [--public-key PATH]`.
  It checks size, SHA-256, schema, signature, duplicates, manifest hashes and
  the registry scan without importing. Unsigned + supplied key is rejected.
- Tests: two `skill_verify_cli` e2e tests for unsigned/signed bundles and a
  tampered bundle (`cargo test --test skill_verify_cli`).
- CUA provenance policy (ADR-0053): `cua::evaluate` tags action arguments as
  plan/trusted/untrusted, denies high-risk actions with untrusted arguments,
  requires approval for other untrusted or irreversible actions, and denies a
  planner that sees untrusted content. `cua::check_branch` blocks branch
  steering to targets outside the approved plan. `1agent cua-check` exposes it.
- Tests: five policy unit tests plus two `cua_check_cli` e2e tests
  (`cargo test cua --lib`, `cargo test --test cua_check_cli`).
- Cascade containment metrics (ADR-0054): `cascade::simulate` walks a directed
  failure graph with containment points and reports affected/contained/
  propagated, `containment_rate_bps`, `cascade_depth` and `blast_radius`;
  `cascade::evaluate` gates them. `1agent cascade-check` exposes it. This is a
  deterministic simulation, not a live multi-agent run.
- Tests: four cascade unit tests plus three `cascade_check_cli` e2e tests
  (`cargo test cascade --lib`, `cargo test --test cascade_check_cli`).
- Budget policy (ADR-0055): `budget::evaluate` enforces optional hard limits
  on cost/tokens/tool calls (exceeded fails, 80% warns) and
  `cost_per_verified_outcome` returns `None` when nothing verified.
  `1agent budget-check` exposes it and exits non-zero on an exceeded budget.
  This is a policy/CI helper; runtime enforcement is pending.
- Tests: four budget unit tests plus two `budget_check_cli` e2e tests
  (`cargo test budget --lib`, `cargo test --test budget_check_cli`).
- Runtime budget enforcement: `RunBudget` gains `max_cost_micros` and
  `max_tokens` (old budgets default to unlimited). The runtime accumulates
  provider-reported cost and input+output tokens after each response and stops
  with `StopReason::BudgetExhausted` when either limit is exceeded.
  `1agent run --max-cost-micros/--max-tokens` sets them.
- Tests: cost and token budget enforcement in the runtime
  (`cargo test budget_stops --lib`).
- OTel GenAI-style span projection (ADR-0056): `otel::spans_from_events`
  emits a deterministic `invoke_agent` root, `chat` spans with
  provider/model/token/cost attributes and `execute_tool` spans from the
  journal. `1agent otel --journal PATH [--json]` prints them. This is a JSON
  projection, not an OTel SDK/exporter.
- Tests: two OTel unit tests plus one `otel_cli` e2e test
  (`cargo test otel --lib`, `cargo test --test otel_cli`).
- Chat adapter contract (ADR-0057): `to_task_request` maps a `ChatMessage` to
  a `TaskRequest`; `format_reply` returns the conclusion, one trust line,
  `needs_approval` and `inbox_open`, without schema field names. Transport
  implementations are separate.
- Tests: three `chat` unit tests (`cargo test chat --lib`).
- Team role policy (ADR-0058): `team::authorize` enforces owner/approver/
  member/viewer against read/write/approve/manage and a scope; empty scopes
  mean all; unknown actors, disallowed actions and ungranted scopes are denied.
  `team::intersect_scopes` supports delegation; `1agent team-check` exposes it.
  This is policy, not an identity provider.
- Tests: four team unit tests plus two `team_check_cli` e2e tests
  (`cargo test team --lib`, `cargo test --test team_check_cli`).
- Agent Card signing (ADR-0059): `--generate-key` creates an Ed25519 PKCS#8
  key pair; `--sign-key` signs the canonical unsigned payload; `--verify PATH
  --trusted-key HEX` requires a matching embedded key and a valid signature.
  Tampered, wrong-key and unsigned cards are rejected. The card advertises
  `signed_agent_card` as verified. This is a local trust statement, not A2A
  registry discovery.
- Tests: three agent-card unit tests plus one `agent_card_cli` signing e2e
  test (`cargo test agent_card --lib`, `cargo test --test agent_card_cli`).
- Durable idempotency records (ADR-0060): `IdempotencyStore` persists
  key/fingerprint/response as append-only JSON lines, `sync_data`s before
  acknowledging, and replays across restarts. The same key with a different
  fingerprint conflicts. A final line without a newline is ignored as an
  interrupted append. `1agent idempotency record|replay` exposes it. This is
  the cross-restart dedupe primitive, not yet wired into every external write.
- Tests: six idempotency unit tests plus one `idempotency_cli` e2e test
  (`cargo test idempotency --lib`, `cargo test --test idempotency_cli`).
- Out-of-band daemon watchdog (ADR-0061): `daemon --heartbeat PATH` writes an
  atomic `{pid, updated_at_ms}` heartbeat each loop. `1agent watchdog
  --heartbeat PATH --max-age-ms N` exits non-zero when the heartbeat is
  missing, stale or invalid. It observes liveness only; it does not restart the
  daemon or clear its lock.
- Tests: four watchdog unit tests plus three `watchdog_cli` e2e tests
  (`cargo test watchdog --lib`, `cargo test --test watchdog_cli`).
- Autonomy quiet hours (ADR-0062): `QuietHours` parses `HH:MM-HH:MM` windows
  that may cross midnight. `decide_proactive` turns proactive
  act/require_approval into `defer` inside the window and reports minutes until
  it ends; user-initiated work and explicit approval are never deferred.
  `1agent autonomy --quiet ... --now ... [--user-initiated]` exposes it.
- Tests: two autonomy unit tests plus one `autonomy_cli` e2e test
  (`cargo test autonomy --lib`, `cargo test --test autonomy_cli`).
- False-alarm budget (ADR-0063): `alarm_budget::evaluate` delivers ordinary
  alerts while the delivery budget and false-alarm ceiling hold; otherwise it
  moves them to a digest. Critical alerts bypass both. `1agent alarm-budget`
  exposes it. Policy only; delivery, persistence and acknowledgement are
  separate.
- Tests: five alarm-budget unit tests plus two `alarm_budget_cli` e2e tests
  (`cargo test alarm_budget --lib`, `cargo test --test alarm_budget_cli`).
- Escalation ladder (ADR-0064): `EscalationLadder` holds strictly increasing
  `(after_ms, action)` steps (`inbox`/`digest`/`notify`/`urgent`). `plan`
  selects the highest reached rung, returns no level before the first rung or
  for a resolved item. `1agent escalate` exposes it. It computes a level; it
  does not send or mutate the item.
- Tests: four escalation unit tests plus two `escalate_cli` e2e tests
  (`cargo test escalation --lib`, `cargo test --test escalate_cli`).
- Organization audit index (ADR-0065): `OrgAuditStore` appends
  actor/action/scope/decision entries with seq, previous hash and a SHA-256
  hash over a canonical body; every append is synced. `verify` detects edited
  fields, reordering and broken links. `1agent org-audit append|verify` exposes
  it. This is a local index, not a remote transparency log or an IdP.
- Tests: four org-audit unit tests plus two `org_audit_cli` e2e tests
  (`cargo test org_audit --lib`, `cargo test --test org_audit_cli`).
- MCP OAuth claims policy (ADR-0066): `mcp_auth::authorize` checks exact
  audience, expiry/not-before with clock skew, a maximum TTL and an exact
  required scope over claims the caller already signature-verified.
  `1agent mcp-token-check` reads claims from a file, never argv, and never
  prints the subject on rejection. HTTP/SSE transport is still absent.
- Tests: four mcp-auth unit tests plus three `mcp_token_cli` e2e tests
  (`cargo test mcp_auth --lib`, `cargo test --test mcp_token_cli`).
- MCP streamable HTTP transport (ADR-0070): MCP servers may set exactly one of
  `command` or `url`. HTTP requires HTTPS except for loopback, rejects embedded
  credentials, carries `Mcp-Session-Id`, parses JSON or `text/event-stream`
  responses, and reads a bearer token from the environment variable named by
  `bearer_token_env`. Existing stdio configs are unchanged.
- Tests: three MCP HTTP/SSE unit tests plus the existing stdio tests
  (`cargo test tools::mcp --lib`).
- Cost per verified outcome (ADR-0071): `value-report --verified-outcomes N`
  adds `verified_outcomes` and `cost_per_verified_outcome_micros` to the
  report. The count is caller/verifier-supplied; the report does not infer it
  from completed runs.
- Tests: value-report unit and CLI tests cover the supplied count and zero-cost
  output (`cargo test value_report --lib`, `cargo test --test value_report_cli`).
- Windows cross-check: `RUSTFLAGS="-D warnings" cargo check --target
  x86_64-pc-windows-gnu` passes, including all test targets. `read_key_hex`
  now consumes its `secret` flag on non-Unix targets; Unix-only test modules,
  cases and imports are cfg-gated. The macOS arm64 check is blocked by the host
  C toolchain for `ring`, so macOS remains unverified.
- Evidence: `docs/evidence/cross-check-windows-2026-11-12.md`.
- Trust Center verification summary: `trust-center --verification-store PATH`
  includes the local all-time verified/hacked/rejected/bps summary in the
  disclosure. It remains a local claim, not an external audit.
- Tests: one trust-center unit test and one `trust_center_cli` e2e test cover
  the summary (`cargo test trust_center --lib`,
  `cargo test --test trust_center_cli`).
- Digest verification summary: `digest --verification-store PATH` adds the
  all-time local verified/hacked/rejected/bps summary.
- Tests: one `digest_cli` e2e test covers a hacked report
  (`cargo test --test digest_cli`).
- A-Gate soak evidence: five consecutive `a_gate_soak` cycles pass locally
  (`docs/evidence/a-gate-soak-2026-11-12.md`). This is the software soak, not
  the external A-Gate checklist or a 24h multi-agent acceptance run.
- Digest webhook: `digest --notify-webhook URL --notify-format
  generic|slack|discord` posts the same text summary through the one-way
  notifier. URL policy and empty-text rejection are shared.
- Tests: two `digest_notify_cli` e2e tests with a loopback webhook
  (`cargo test --test digest_notify_cli`).
- Trust Center Markdown: `trust-center --markdown-out PATH` renders the JSON
  disclosure as a human-readable page. Audit remains `not_audited` and the
  output is not a hosted service.
- Tests: one trust-center unit test plus the `trust_center_cli` e2e test cover
  the Markdown (`cargo test trust_center --lib`,
  `cargo test --test trust_center_cli`).
- MCP per-server timeout: `timeout_seconds` (1..=300, default 20) bounds stdio
  reads, HTTP requests and the tool execution timeout. Out-of-range values
  fail closed.
- Tests: the MCP config test covers timeout 0 and 301
  (`cargo test tools::mcp --lib`).
- MCP response cap: HTTP bodies are streamed with a 2 MiB cap and must be
  UTF-8; stdio lines over the cap fail closed. A malicious or broken server
  cannot force unbounded response allocation.
- Tests: two MCP unit tests cover the HTTP cap and the bounded stdio line
  reader (`cargo test tools::mcp --lib`).
- Organization audit listing: `org-audit list --limit N [--json]` shows the
  tail entries for operator review.
- Tests: one `org_audit_cli` tail-listing test
  (`cargo test --test org_audit_cli`).
- Cross-check evidence now records that the MSVC attempt is blocked by the
  host lacking `lib.exe`, alongside the macOS host-toolchain block; the GNU
  target remains the verified Windows compile.
- Handoff filters: `handoff list --open --to NAME` filters the list by status
  and recipient while preserving creation-time order.
- Tests: the `handoff_cli` test covers open and recipient filters
  (`cargo test --test handoff_cli`).
- Verification summary window: `verification summary --since-ms/--until-ms`
  applies the existing `summary_between` filter; without flags it remains
  all-time.
- Tests: the `verification_store_cli` test covers a windowed summary
  (`cargo test --test verification_store_cli`).
- Inbox filters: `inbox list --kind KIND --source SOURCE` filters the open
  queue; an invalid kind fails closed.
- Tests: one `inbox_cli` filter test (`cargo test --test inbox_cli`).
- Value report open handoffs: `value-report --handoff-store PATH --recipient
  NAME` adds `handoffs_open`; the recipient requires the store.
- Tests: one `value_report_cli` handoff test
  (`cargo test --test value_report_cli`).
- Inbox notification dry-run: `inbox-notify --dry-run` reports open/new counts
  without calling the webhook, writing receipts or advancing the watermark.
- Tests: one `inbox_notify_cli` dry-run test
  (`cargo test --test inbox_notify_cli`).
- Chat bridge lock: `chat-bridge ingest --lock PATH` fails if the lock exists
  and removes it on normal exit. A crash leaves a stale lock for manual
  cleanup.
- Tests: one `chat_bridge_cli` lock test
  (`cargo test --test chat_bridge_cli`).
- Agent Card now discloses the trust-center capability as verified.
- Tests: the agent-card unit test asserts it
  (`cargo test agent_card --lib`).
- Handoff cancellation (ADR-0068): `handoff cancel --id ... --actor ...
  [--reason ...]` lets the sender or addressee cancel an open handoff. A
  cancelled handoff is terminal and cannot be accepted; the list shows it and
  `--open` excludes it.
- Tests: two handoff unit tests plus one `handoff_cli` cancel test
  (`cargo test handoff --lib`, `cargo test --test handoff_cli`).
- Inbox reconciliation: `inbox reconcile --handoff-store PATH [--apply]`
  closes projected handoff items as approved/dismissed when the handoff is
  accepted/cancelled. Open handoffs are untouched; dry-run is the default.
- Tests: one inbox unit test plus one `handoff_inbox_cli` reconcile test
  (`cargo test inbox --lib`, `cargo test --test handoff_inbox_cli`).
- Inbox escalation projection: `1agent inbox escalate --step AFTER_MS=ACTION
  ... [--now-ms ...] [--json]` applies the existing ladder to every open or
  due-snoozed item using its `created_ms`. Terminal items are excluded; it
  reports levels and reasons without sending or changing status.
- Escalation delivery: with `--state PATH` and `--webhook URL` or repeatable
  `--target FORMAT=URL`, newly reached `notify`/`urgent` rungs are delivered
  in one batch (at most 50 items) and the per-item watermark advances only
  after every target succeeds. Targets share one state and are sent in order;
  a later failure can duplicate one batch to earlier targets on retry.
  Resolved items are pruned; `--dry-run` reports without sending. A crash
  between send and save can duplicate one batch; digest-rung delivery is not
  implemented. The daemon accepts the same `--escalate-target FORMAT=URL`
  fan-out.
- Tests: five `inbox_escalate_cli` e2e tests cover open-vs-resolved items, the
  below-first-threshold case, invalid ladders, one delivery per reached rung
  including a raised level, and multi-target fan-out (`cargo test --test
  inbox_escalate_cli`). The daemon e2e test now uses two escalation targets.
- Daemon escalation delivery: `daemon --escalate-inbox-store PATH
  --escalate-step ... --escalate-webhook URL --escalate-state PATH
  [--escalate-format ...]` runs the same projection and delivery on every
  tick. Delivery failures are logged and retried next tick; quiet hours
  suppress delivery. Shared as `inbox_escalation::run` for the CLI and daemon.
- Tests: three `daemon_escalation_cli` e2e tests cover per-rung delivery,
  quiet-hours suppression and partial configuration rejection (`cargo test
  --test daemon_escalation_cli`).
- Digest escalation summary: `digest --escalation-step AFTER_MS=ACTION ...`
  adds per-level counts (including the `digest` rung) and the item projection
  to the text/JSON digest. Without the flag the digest is unchanged.
- Tests: one `digest_cli` e2e test covers the level counts and the unflagged
  case (`cargo test --test digest_cli`).
- Quiet-hours boundary wake: `QuietHours::ms_until_end` measures milliseconds
  to the exclusive window end in a fixed UTC offset. The daemon's quiet-period
  sleep is `min(poll interval, ms_until_end)`, so deferred due jobs re-check at
  the boundary instead of only after a full poll; the heartbeat cadence is
  unchanged.
- Tests: `quiet_hours_ms_until_end_tracks_the_boundary` covers the overnight
  boundary, a 30-second remainder and a fixed offset (`cargo test autonomy
  --lib`).
- Agent Card disclosure: verified capabilities now name multi-target inbox
  notifications, inbox escalation delivery, repair-mark and the opt-in daemon
  maintenance loop; the trust-center capability counts follow the card.
- Tests: `agent_card_cli` asserts the four capability ids are verified and the
  notification note mentions multi-target fan-out.
- Daemon inbox maintenance: `daemon --reconcile-inbox-store PATH
  --reconcile-handoff-store PATH --reconcile-apply` runs the existing
  terminal-handoff reconcile on every tick, closing accepted handoffs as
  approved and cancelled ones as dismissed. Quiet hours skip it; the three
  flags must appear together.
- Tests: two `daemon_reconcile_cli` e2e tests cover automatic reconciliation
  and incomplete-configuration rejection (`cargo test --test
  daemon_reconcile_cli`).
- Daemon alert projection: with `--alert-log PATH` and `--alert-inbox-store
  PATH`, every tick projects new alert records into the inbox and applies the
  sync (item-id dedupe, existing statuses preserved). Quiet hours skip it.
- Tests: two `daemon_alert_inbox_cli` e2e tests cover projection/dedupe and
  the missing-log rejection (`cargo test --test daemon_alert_inbox_cli`).
- Daemon goal projection: with `--goal-journal PATH` and `--goal-inbox-store
  PATH`, every tick projects overdue goals/commitments into the inbox using
  the default attention policy and sync's item-id dedupe. The two flags are
  required together; quiet hours skip it.
- Tests: two `daemon_goal_inbox_cli` e2e tests cover projection/dedupe and
  flag pairing (`cargo test --test daemon_goal_inbox_cli`).
- Stale attention cleanup: `inbox::reconcile_attention` dismisses open
  `attention:` items absent from the current candidate set. The daemon runs it
  after the goal sync; the dry-run mode is library-only. A dismissed item is
  not re-added by sync if the condition returns because its item id is stable;
  a recurrence needs a fresh item id. (`InboxStore::reopen` only transitions
  open/snoozed items, not terminal ones.)
- Tests: one inbox unit test covers dry-run/apply and non-attention isolation;
  the daemon goal e2e test now closes the commitment and asserts the item is
  dismissed on the next tick (`cargo test inbox --lib`, `cargo test --test
  daemon_goal_inbox_cli`).
- TUI `/receipt`: displays the most recent reframe decision receipt from the
  current run journal or, failing that, the newest session journal. It uses
  the same replay and `render_receipt` path as the CLI.
- Tests: the TUI command registry test covers `/receipt` registration and the
  command-count expectations (`cargo test tui_commands --lib`).
- Receipt index: `receipt-index --journal-dir DIR [--json]` scans decision
  journals and lists run id, journal, stated request, choice, actor and
  decision time, newest first, capped at 200. Missing or unrelated journals
  are skipped. This is the CLI retrieval path; TUI `/receipt` remains future
  work.
- Tests: two `receipt_index_cli` e2e tests cover newest-first ordering and a
  missing directory (`cargo test --test receipt_index_cli`).
- Reframe repeat guard: `reframe` scans the newest 500 decision journals; if
  the same `stated_request` was rejected before, it reports
  `repeated_rejection` and does not call the model. `--force` overrides. The
  scan does not write a journal.
- Tests: one `reframe_repeat_cli` e2e test covers rejection, repeat suppression,
  `--force` and a different request (`cargo test --test reframe_repeat_cli`).
- MCP token exchange: `mcp-token-exchange --endpoint URL --audience A --scope
  S --subject-token-env VAR [--client-id-env VAR --client-secret-env VAR]
  [--max-ttl-seconds N] --out PATH [--json]` performs an RFC 8693-style form
  exchange and writes the bearer token to a new 0600 file. Secrets are read
  only from named environment variables; the endpoint must be HTTPS or
  loopback, redirects are refused, the response is capped at 1 MiB, and
  stdout reports only type/TTL/SHA-256 (ADR-0083). The file can feed
  `McpServerConfig.bearer_token_file`; automatic refresh is not implemented.
- Tests: three `mcp_token_exchange_cli` e2e tests cover form fields, 0600
  output, non-disclosure, missing env, insecure endpoint, wrong type,
  excessive TTL, partial client credentials and no-overwrite
  (`cargo test --test mcp_token_exchange_cli`).
- MCP bearer-token files: `McpServerConfig` accepts `bearer_token_file` as an
  alternative to `bearer_token_env` (never both). The file must be a regular
  non-symlink file of 1..=4096 bytes, 0600 on Unix, containing one non-empty
  token; the token is never logged. This lets an operator rotate a short-lived
  token file without changing the process environment.
- Tests: MCP unit tests assert the file token reaches the HTTP `Authorization`
  header and reject loose permissions, symlinks, empty/whitespace tokens and
  both sources (`cargo test tools::mcp --lib`).
- Trust Center HTML output: `trust-center --html-out PATH` writes a static,
  script-free disclosure page. Capability counts, limits, audit, data policy,
  verification, evaluations and contacts are HTML-escaped; contact URLs are
  rendered as text rather than links.
- Tests: one unit and one `trust_center_cli` e2e test cover escaping
  (`<script>`/`javascript:`) and the static-page disclaimer
  (`cargo test trust_center --lib`, `cargo test --test trust_center_cli`).
- Subagent permission intersection: `spawn_agent` accepts `allow_write` and
  `allow_network` booleans. Each is intersected with the parent's permission,
  so a child can only be more restrictive; the result is returned in the
  child's JSON content. Tests cover requested restriction and a widening
  attempt against a read-only parent (`cargo test --offline --lib
  tools::subagent`).
- Notification receipt summary: `notify-receipts --log PATH [--json]` reads a
  mixed inbox/digest receipt log and reports total/inbox/digest/last_sent_ms/
  format counts. Malformed or unknown lines fail closed; a missing log is
  empty. It does not verify real delivery.
- Tests: three `notify_receipts_cli` e2e tests cover mixed logs, malformed and
  unknown lines, and the missing-file case (`cargo test --test
  notify_receipts_cli`).
- Digest receipts: `digest --notify-log PATH` appends one JSONL receipt after
  a successful `--notify-webhook` send (sent_at_ms/runs/alerts/inbox_open/
  format, no URL). The flag requires the webhook.
- Tests: one `digest_notify_cli` e2e test covers the receipt and the
  missing-webhook rejection (`cargo test --test digest_notify_cli`).
- OTLP/HTTP JSON export: `1agent otel-export --journal PATH --endpoint URL
  [--service-name NAME] [--token-env VAR] [--json]` wraps the journal-derived
  GenAI spans in a `resourceSpans`/`scopeSpans` envelope and posts to a
  collector. HTTPS or loopback HTTP only, no redirects, 5s timeout,
  environment-only bearer token, no token/endpoint echo. Status/OK/ERROR map to
  OTLP codes 1/2; attributes are string values. This is a wire adapter, not an
  OTel SDK/gRPC exporter (ADR-0082).
- Tests: four `otel_export_cli` e2e tests cover the envelope shape,
  environment-only auth, missing token, insecure endpoint, redirect refusal and
  a non-2xx collector response (`cargo test --test otel_export_cli`).
- Inbox reopen: `1agent inbox reopen --store PATH --item-id ID [--json]`
  returns an open/snoozed item to open, cancelling a snooze; terminal items
  still fail closed. The TUI `/inbox reopen <id>` uses the same action parser
  and apply path. Tests extend `inbox_cli` and the inbox unit suite (`cargo
  test --test inbox_cli`, `cargo test inbox --lib`).
- Repair marking: `1agent repair-mark --store PATH --ticket-id ID
  --side-effect-id ID --actor NAME --evidence TEXT [--json]` records a
  human-performed compensation by appending a `SideEffectStatusChanged` event
  whose evidence includes the actor. Applied effects are marked as a manual
  compensation; failed effects can be cleared after a human verifies no partial
  effect remains, with evidence recorded as `failed-effect clearance by ...`.
  Repeated marks, wrong tickets, unknown and planned effects fail closed. The
  command does not execute the compensation or verify the claim.
- Ledger transition: `failed -> compensated` is now allowed for that verified
  clearance path; `failed -> applied` remains the retry path.
- Tests: a task-graph unit test covers failed-to-compensated replay, and a
  second `repair_mark_cli` e2e test covers clearing a failed effect to a clean
  runbook (`cargo test task_graph --lib`, `cargo test --test repair_mark_cli`).
- Tests: one `repair_mark_cli` e2e test covers the full flow, the now-clean
  runbook, repeated marking, wrong ticket, unknown id and empty evidence
  (`cargo test --test repair_mark_cli`).
- Multi-target inbox notifications: `inbox-notify --target FORMAT=URL` may
  repeat (mutually exclusive with `--webhook`), letting one run fan out to
  several webhooks with per-target generic/Slack/Discord formats. Targets share
  one watermark; a later failure stops the fan-out without advancing it, and
  the caller's retry can duplicate one message to earlier targets. One receipt
  per target is appended on success. Exact duplicate `--target` entries are
  rejected.
- Tests: two `inbox_notify_targets_cli` e2e tests cover per-target payload
  formats, shared-watermark suppression and partial-failure state retention
  (`cargo test --test inbox_notify_targets_cli`).
- Webhook redirect hardening: the inbox notifier, alert webhook and MCP HTTP
  client no longer follow redirects, so a loopback/HTTPS endpoint cannot bounce
  a request to a private/metadata host.
- Tests: one notifier unit test serves a 302 and asserts no second connection
  (`cargo test notify --lib`).
- Repair runbook: `repair::build_runbook` maps side effects to plans, errors
  and compensate/manual counts; `1agent repair-plan --store ...
  [--require-clean]` exposes it and exits non-zero while repairs remain.
  Execution still requires Authority/Policy/Approval.
- Tests: one repair unit test plus three `repair_plan_cli` e2e tests,
  including a real `task-graph demo` side effect that fails `--require-clean`
  (`cargo test repair --lib`, `cargo test --test repair_plan_cli`).
- One-way inbox presence (ADR-0080): `1agent inbox-notify` posts open inbox
  items to a generic/Slack/Discord webhook. HTTPS is required except for
  loopback; embedded credentials are rejected; the payload is bounded and an
  empty queue sends nothing. It does not read replies or mutate items.
- Tests: three notifier unit tests plus two `inbox_notify_cli` e2e tests with
  a loopback webhook (`cargo test notify --lib`,
  `cargo test --test inbox_notify_cli`).
- Inbox notification watermark: `--state PATH` stores `last_sent_ms` and
  suppresses repeats until an item is newer. The watermark advances only after
  a successful send; a failed send leaves it unchanged.
- Inbox notification receipts: `--log PATH` appends one JSONL receipt per
  successful send with time/open/new_items/format; the URL is not stored.
- Daemon quiet hours: `daemon --quiet-hours HH:MM-HH:MM
  --utc-offset-minutes N` skips due jobs inside the window. Jobs remain due and
  run after the window; the watchdog heartbeat still advances.
- Tests: two `daemon_quiet_cli` e2e tests
  (`cargo test --test daemon_quiet_cli`).
- Agent Card capability disclosure: adds verified entries for
  `verification_store`, `team_handoff`, `inbox_notifications`,
  `repair_runbook`, `org_audit` and `mcp_http_transport`.
- Tests: the agent-card unit test asserts the new entries are verified
  (`cargo test agent_card --lib`).
- File chat bridge (ADR-0081): `chat-bridge task` converts a validated
  `ChatMessage` JSON to a `TaskRequest` file; `chat-bridge reply` converts a
  `RunResult` JSON to a `ChatReply` file. Inputs are bounded to 1 MiB and
  regular non-symlink files; writes are atomic. This is an adapter building
  block, not a Slack/Telegram/WeChat connection.
- Tests: three chat-bridge unit tests plus three `chat_bridge_cli` e2e tests
  (`cargo test chat_bridge --lib`, `cargo test --test chat_bridge_cli`).
- Chat bridge spool: `chat-bridge ingest --inbox DIR --workspace DIR --out DIR`
  processes up to 500 top-level JSON messages, writes task files, moves valid
  messages to `processed/` and invalid ones to `failed/` with an error file.
  Any failed message makes the command exit non-zero. An existing
  `<stem>.task.json` is skipped and its message is moved to `processed/`, so a
  retried ingest after a move failure does not overwrite the task. `--state
  PATH` stores message SHA-256 values (up to 10,000) and skips renamed
  duplicates.
- Team audit linkage: `team-check --audit-store PATH --actor-id NAME` appends
  the allow/deny verdict (including denials) to the organization audit chain
  with action/scope.
- Tests: one `team_check_cli` e2e test verifies two entries and a valid chain
  (`cargo test --test team_check_cli`).
- Onboarding context import (ADR-0072): `onboard --import-file PATH
  --import-kind fact|preference|project|feedback` writes one memory per
  non-empty, non-comment line. Files must be regular, not symlinks, ≤256 KiB,
  ≤200 usable lines and ≤2000 chars per line; the import is not redacted.
- Tests: two `onboard_cli` import tests
  (`cargo test --test onboard_cli`).
- Inbox handoff acceptance (ADR-0073): `inbox resolve handoff:<id> approved`
  requires `--handoff-store PATH --actor NAME`. It accepts the handoff before
  closing the inbox item; a wrong actor, terminal handoff or missing store
  fails closed. Other terminal statuses only close the inbox item.
- Tests: one `handoff_inbox_cli` acceptance test
  (`cargo test --test handoff_inbox_cli`).
- Organization audit seals (ADR-0074): `org-audit generate-key` writes a
  PKCS#8 Ed25519 private key file and a public key; `seal` signs the entry
  count and head hash; `verify --seal ... --public-key ...` checks the chain
  and seal together. A new entry after sealing, a changed head or a wrong key
  fails. The seal is only as strong as the out-of-band trust in the public key.
- Tests: one org-audit seal unit test plus one `org_audit_seal_cli` e2e test
  (`cargo test org_audit --lib`, `cargo test --test org_audit_seal_cli`).
- Digest additions (ADR-0075): `digest` now reports open inbox items and, with
  `--handoff-store PATH --recipient NAME`, open handoffs addressed to that
  person. `--recipient` without a handoff store fails closed.
- Tests: two `digest_cli` e2e tests (`cargo test --test digest_cli`).
- Trust disclosure manifest v0 (ADR-0076): `1agent trust-center` derives
  capability counts, limits and the data policy from the Agent Card, accepts
  caller-supplied evaluation disclosures, and always states
  `audit.status = not_audited` with `external_audit = false`. It writes a JSON
  manifest; it is not a hosted Trust Center page or a third-party audit.
- Tests: three trust-center unit tests plus two `trust_center_cli` e2e tests
  (`cargo test trust_center --lib`, `cargo test --test trust_center_cli`).
- Eval disclosure bridge: `1agent eval --disclosure-out PATH` writes an
  `EvaluationDisclosure` with the suite name, package version, passed/total and
  the SHA-256 of the pretty-printed report. `trust-center --evaluation` accepts
  the file directly.
- Tests: one `eval_disclosure_cli` e2e test
  (`cargo test --test eval_disclosure_cli`).
- Unit economics calculator (ADR-0077): `unit_economics::evaluate` reports
  contribution, net, gross-margin bps, per-outcome revenue/cost and break-even
  outcomes from caller-supplied revenue and costs. `1agent unit-economics`
  exposes it. No pricing policy or price points are built in.
- Tests: three unit-economics unit tests plus two `unit_economics_cli` e2e
  tests (`cargo test unit_economics --lib`,
  `cargo test --test unit_economics_cli`).
- Agent Card discovery route (ADR-0078): `GET
  /.well-known/agent-card.json` serves the same unsigned payload as
  `/v0/agent-card` without a bearer token. This is a discovery path, not A2A
  registry discovery or a signed registry entry.
- Tests: the HTTP surface unit test covers both public card routes
  (`cargo test http_surface --lib`).
- MCP audit metadata: tool outcomes now carry both `mcp_server` and `mcp_tool`
  so the journal identifies the server that produced the result.
- Tests: the MCP stdio test asserts both metadata fields
  (`cargo test tools::mcp --lib`).
- Quiet-hours persistence: `onboard --quiet-hours HH:MM-HH:MM` writes a
  `quiet_hours=` preference; `UserModel` exposes it as `quiet_hours`.
  `1agent user-model` and the onboard JSON report include the window.
- Tests: the user-model parsing test and one `onboard_cli` test cover the
  window, text/JSON output and invalid input
  (`cargo test user_model --lib`, `cargo test --test onboard_cli`).
- Autonomy from the UserModel: `autonomy::decide_for_proactive` reads the
  domain level and persisted quiet hours. `1agent autonomy-for --domain ...
  --now ...` exposes it; user-initiated work and explicit approval still bypass
  quiet deferral.
- Tests: one `autonomy_cli` e2e test covers defer, daytime act and
  user-initiated act (`cargo test --test autonomy_cli`).
- Agent Card key files: `agent-card --generate-key --private-out PATH
  --public-out PATH` writes the private key as a new 0600 file and stops
  printing it; `--sign-key-file PATH` signs from that file. `--sign-key` remains
  for compatibility but exposes the key in argv. Tests use the file path.
- Tests: one `agent_card_cli` key-file test; `card_trust_cli` now signs from a
  file (`cargo test --test agent_card_cli`, `cargo test --test card_trust_cli`).
- Verification store (ADR-0079): `VerificationStore` appends one report per
  verification id, rejects duplicate ids/self-verification, reuses
  `verification::summarize`, and counts clean Verified outcomes in a window.
  `1agent verification record|summary` exposes it; `value-report
  --verification-store` derives the verified count automatically and conflicts
  with `--verified-outcomes`.
- Tests: three verification-store unit tests plus one
  `verification_store_cli` e2e test (`cargo test verification_store --lib`,
  `cargo test --test verification_store_cli`).
- Verification failures reach the inbox: `verification record --inbox-store
  PATH` adds a `verification:<id>` review item for Hacked/Rejected outcomes;
  Verified outcomes do not create an item and sync dedupes by id.
- Tests: the `verification_store_cli` test checks one review item for the
  Hacked report and none for the Verified report.
- Value report verification summary: with `--verification-store`, the report
  also includes the window's `verified/hacked/rejected/resolved/
  hacked_resolved_bps` summary. `summary_between` filters by record time.
- Tests: the `verification_store_cli` test asserts the summary fields.
- Cross-target gate script: `scripts/cross-check.sh` requires the Windows GNU
  `-D warnings` all-targets check and attempts macOS arm64, reporting a host
  C-toolchain block unless `--require-macos` is passed.
- Evidence: `docs/evidence/cross-check-windows-2026-11-12.md`.
- Agent Card trust store (ADR-0067): `CardTrustStore` keeps trusted Ed25519
  public keys with add/revoke timestamps. `verify` requires the card's embedded
  key to be trusted and unrevoked, then delegates to `agent_card::verify_card`.
  `1agent card-trust add|revoke|list|verify` exposes it. The store is a local
  trust anchor, not remote key discovery.
- Tests: four card-trust unit tests plus two `card_trust_cli` e2e tests
  (`cargo test card_trust --lib`, `cargo test --test card_trust_cli`).
- Team handoff (ADR-0068): `HandoffStore` appends grounded handoff snapshots
  to JSONL. `create` requires distinct from/to, a non-empty summary and at
  least one evidence reference; only the addressee may accept, and a terminal
  handoff cannot transition again. `1agent handoff create|accept|list` exposes
  it. The Agent Card moves team collaboration from unsupported to degraded.
- Tests: four handoff unit tests plus two `handoff_cli` e2e tests
  (`cargo test handoff --lib`, `cargo test --test handoff_cli`).
- Handoff inbox projection (ADR-0069): `project_handoffs` maps open handoffs
  addressed to the recipient to `review` items with `handoff:<id>`;
  `inbox sync --handoff-store PATH --recipient NAME` merges them with the same
  dry-run/dedupe contract as the other projections. It never accepts a handoff.
- Tests: one inbox unit test plus two `handoff_inbox_cli` e2e tests
  (`cargo test inbox --lib`, `cargo test --test handoff_inbox_cli`).
- Tool search v0 (ADR-0049): `tool_search::search` ranks tools by exact name,
  name prefix, name token, description token and parameter-name token, with
  deterministic tie-breaking. `1agent tools --query TEXT` exposes it. This is
  a pure ranking function; it does not yet change the model-visible tool list.
- Tests: four tool-search unit tests plus two `tool_search_cli` e2e tests
  (`cargo test tool_search --lib`, `cargo test --test tool_search_cli`).
- Tool search request wiring (ADR-0050): `AgentRuntime::with_tool_query` and
  `1agent run --tool-query TEXT` advertise the top 8 hits plus the always-on
  file/terminal tools; the selected list is journaled in `ModelRequested`.
  Without a query, the full permission-filtered list is unchanged.
- Tests: two selection unit tests and one `tool_query_cli` e2e test for the
  journaled tool list (`cargo test tool_search --lib`,
  `cargo test --test tool_query_cli`).

## 0.96.1 - 2026-11-11

TUI, context-budgeting and prompt-library update on the R1 candidate. This is a
patch release; it does not claim the 0.97 Reliable Loop scope.

### Answer readability

- Conclusions now render Markdown hierarchy inside the `结论 Conclusion` fence:
  `##` subheadings, ordered and unordered lists, `- [x]` task boxes, bold key
  labels, `---` dividers, link-colored URLs/local paths and column-aligned
  tables.
- The system prompt asks for one result sentence first, then short paragraphs,
  bullets or a comparison table for multi-dimensional answers; simple answers
  stay one short paragraph. A live `deepseek-flash` canary returned a capability
  table, bullets and a defaulted question instead of one wall of text.
- URLs glued to Chinese text (for example `参考：https://...`) are link-colored.

### Prompt library

- `/` still searches the 48 curated Chinese cards and shows three featured
  prompts on an empty query.
- `//` opens the complete source table: the 56 verbatim Feishu rows are grouped
  into 32 author/topic entries and labeled by category. The snapshot and
  importer are `docs/evidence/prompt-sources-2026-11-11.ndjson` and
  `scripts/import-prompt-sources.py`.
- Source insertion is body-only: the complete Chinese translation in Chinese
  mode or the complete English original in English mode. No author/topic header
  and no reference line are written into the composer; author, topic, category
  and link stay in the drawer row. Inserting the same body twice is skipped.
- On wide terminals a side pane previews the selected group's complete body,
  author, category, fragment count and character count before insertion.
- `提示词语言 中文 / English` setting (`/config set language zh|en`). The
  curated English subset lists only the 14 cards with verbatim English source
  text; the source library is available in both languages.
- Completeness gate: `tests/prompt_sources.rs` reparses the 56-row snapshot and
  compares every group's joined original/translation and row count, so import
  truncation or rewriting fails the build.
- Follow-up turns inject the previous request/answer and prompt-library facts as
  a clean `CONVERSATION CONTEXT` message; a short reply such as `felix` is
  resolved against the previous question instead of starting a new search.

### Context and budgets

- Token-based context budget: default 1,000,000 tokens, automatic compaction at
  786,432 tokens; `--max-context-tokens` replaces `--max-context-chars`. Legacy
  character budgets migrate on read (the old 100,000-character default maps to
  1M tokens; custom values use `ceil(chars / 3)`); old session/task JSON remains
  readable.
- TUI, `/status` and `/context` report tokens; provider-reported
  `usage.input_tokens` is authoritative after each response.
- Interactive TUI runs no longer stop at six steps: steps, tool calls, provider
  attempts and run duration default to unlimited (`0`), and `/config set`
  accepts `0`; the status line and `/config` show `不限`. Headless `run`, the
  daemon and subagents keep finite budgets; interactive TUI provider calls keep
  a 300-second transport ceiling.
- Hidden caps removed: the runtime provider-attempt clamp (was 16) and the TUI
  step/tool validation caps (was 64).

### TUI visual system and interaction

- Dark-green backdrop `#06100A` for the whole TUI; session input panel
  `#08140D`, border `#3F644B`, panel text `#487748`, white input text.
- The input-panel top border carries state/step and token/cache/context metrics;
  narrow terminals fold metrics inside the same fence. The bottom border carries
  run status, model and mode. The context meter uses `─` cells (20 wide, 10
  narrow) and keeps the same percentage.
- Welcome wordmark: leading `1` bright green; `Agent` and caption deep green;
  `SKYNET.CLUB` bright green.
- All popups (`/details`, `/help`, the `/` drawer and `Ctrl+T`) share green
  surfaces `#0A1F12`, selected rows `#143D24`, borders `#3F644B` and
  bright-green titles.
- Work log: action names, `•` bullets and links are bright green; details and
  `⎿` results stay deep green `#487748` and unbolded. `第 N 步` counts actual
  model requests; `Ctrl+O` opens run details and the process trace; Bash results
  include a bounded output preview.
- Running state is a two-line activity card driven by the run journal, with
  bilingual stages, model step and a minimal fence. A persistent Master-Agent
  status card covers mission understanding, planning, approval, assignment,
  starting, worker running, collecting, verification and reporting, with a
  next-command footer.
- User messages render on the input-panel background with white text;
  right-click paste is scoped to the composer.
- Composer mouse selection works across the whole composer. Editing is hardened
  against stale/mid-grapheme cursors, with a 64 KiB input cap, a deterministic
  editing fuzz test and a PTY paste/backspace/space fuzz script. TUI panics
  restore the terminal and append to `$HOME/.1a/tui-crash.log`; transient
  input-read errors are retried.
- The final answer is wrapped in a `╭─ 结论 Conclusion ... ╮` fence with dimmed
  deep-green borders, a bright-green bold title, white body and one blank padding
  line. Structured claim/evidence/assumption/constraint/unknown/conflict/flip
  sections use deep green `#487748`; only the conclusion body stays white.
- The status/work marker is the smaller `•`; normal activity uses one cyan
  accent, while green/amber/red are reserved for completed, waiting/retrying and
  blocked outcomes.

### Verification

- `release-check.sh`: fmt, clippy with `-D warnings`, full tests, HTTP e2e, ci
  and TUI interaction, including 400 mixed edit events.
- `verify-claims.sh`: claims table well-formed; see the current run output for
  exact verified/partial/no-evidence counts.
- Real PTY evidence lives in `docs/evidence/tui-2026-11-10/`; answer formatting
  was also checked with a live provider canary.

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
