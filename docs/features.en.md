# Features

> Current version: **0.96.1 R1 release candidate**  
> Validated platform: **Linux x86_64**  
> Labels: **Available (RC)** / **Basic** / **MVP** / **Module** / **Experimental** / **R&D** / **Not implemented**

[简体中文](features.md) · **English**

## 1. Capability map

```text
User
  │
  ├─ 1agent tui / run / serve
  │     │
  │     ├─ tasks & evidence: Trigger / Thread / Ticket / Mandate / Approval / Report
  │     ├─ memory: Working / Episodic / Semantic / Procedural + UserModel
  │     ├─ skills: SKILL.md / versions / scan / evaluation / canary / rollback
  │     ├─ long-lived context: Goal / Commitment / Attention / ContextGraph / Delegation / Evolution
  │     ├─ models: ProviderRegistry / Router / UsageLedger / Shadow
  │     └─ audit: journal / receipt / replay / query / export
  │
  └─ optional: Skynet Workshop
        └─ Session / sandbox / lease / PTY / Artifact / recovery
```

1Agent decides what to do, why, and whether it is done. Workshop decides where and how to execute safely.

## 2. Available today

### 2.1 Interactive TUI (available, RC)

- Single-turn conversation, task runs, status and activity;
- `/` command drawer with categories, recent commands, fuzzy matching and disabled reasons;
- six conversation modes: auto, direct answer, evidence research, decision analysis, execute, diagnosis;
- `Ctrl+T` or `/mode` opens the mode menu;
- `Ctrl+K` opens the command drawer;
- transcript scrolling, mouse selection, copy and cancellation;
- a bare launch starts a new session; exit prints the explicit `--session` continue command;
- running line shows `Working...`, elapsed time, input/output tokens and provider cache hit rate.

See [Manual](manual.en.md) and repository `docs/tui.md`.

### 2.2 Headless runs (available, RC)

```bash
1agent run --task "Analyze this requirement and separate goal from means"
1agent run --task "..." --json
```

Suitable for scripts, CI and batch work. It uses the same runtime, journal, provider and audit contracts as the TUI.

### 2.3 Tasks, authority and approval (available, RC)

- Event → Thread → Gate → Eval → Ticket → Mandate → Assignment → Report;
- AuthorityProfile / PolicyEngine / ExecutionGate;
- one-time approvals, standing approvals and a side-effect ledger;
- retry, takeover, cancel, compensation and reconciliation;
- unauthorized actions are rejected before tool execution;
- independent verification and hack-verifiable acceptance: executor ≠ verifier; a public pass with a hidden failure is `hacked`, not clean; reports expose `hacked_resolved_bps`;
- verification store: `1agent verification record --store PATH --verification-id ID --run-id RUN --input INPUT.json [--inbox-store PATH]` runs verify and appends+syncs the report; Hacked/Rejected outcomes can project a `verification:<id>` review item; `verification summary [--since-ms ... --until-ms ...]` reports verified/hacked/rejected/resolved and `hacked_resolved_bps` with an optional time window; duplicate ids and self-verification are rejected; `value-report --verification-store` counts clean Verified outcomes in the window automatically;
- Trust & Repair: the side-effect ledger projects to applied / compensated / failed / pending / needs_user; an applied reversible action without compensation fails closed; `1agent away-digest` shows what happened while you were away; `1agent repair-plan [--require-clean]` lists compensation/manual/error items and exits non-zero while repairs remain;
- Value report v0: `1agent value-report` summarizes completed / blocked / failed / cancelled runs, tokens/cost, tool calls/failures, alerts, open inbox items, open handoffs for a named recipient and pending decisions; `--verified-outcomes N` or `--verification-store PATH` reports cost per verified outcome (the store counts clean Verified outcomes in the window and adds a verification summary: verified/hacked/rejected/resolved/bps; the flags are mutually exclusive); it reports reproducible outcomes, not an invented time saving;
- Partner voice: operator / advisor / companion; `--voice-mode` selects it; it changes how the answer speaks, not the evidence/authority/action rules; companion does not flatter or role-play;
- Agent Inbox: notify / question / review items; append-only JSONL snapshots; snooze and approve/reject/answer/dismiss state machine; terminal statuses cannot transition; duplicate ids, empty fields and invalid snoozes fail closed; `1agent inbox add|list|resolve|snooze|sync` (`list --kind/--source` filters); pure attention/alert/handoff projections; `sync` dedupes by `item_id`, is dry-run by default and applies with `--apply` without overwriting existing status; open handoffs addressed to `--recipient` project as `handoff:<id>` review items and are never auto-accepted, but `inbox resolve handoff:<id> approved --handoff-store PATH --actor NAME` accepts the linked handoff before closing the item; `inbox reconcile --handoff-store PATH [--apply]` closes projected items for accepted handoffs as approved and cancelled ones as dismissed (dry-run by default);
- Autonomy dial policy: `1agent autonomy --level observe|suggest|act|auto --risk low|medium|high [--reversible] [--explicit-approval] [--quiet HH:MM-HH:MM --now HH:MM] [--user-initiated]`; observe/suggest never act; act allows only low-risk reversible actions; auto allows low/medium risk; high risk or irreversible actions require approval unless explicitly approved. During quiet hours, proactive act/require_approval becomes `defer` with the minutes until the window ends; user-initiated work and explicit approval are never deferred. `onboard --quiet-hours` persists the window, and `autonomy-for --domain ... --now ...` reads the domain level and quiet hours from the UserModel. A preference is not an authorization grant;
- Onboarding v0: `1agent onboard --voice ... --autonomy DOMAIN=LEVEL --boundary TEXT` writes ordinary memory records and prints the resulting `UserModel`; invalid values fail closed. `--goal/--commitment` can seed a first goal and commitment for `inbox sync`; `--interactive` reads voice/goal/commitment/boundary from stdin (prompts on stderr, blank keeps flags); `--quiet-hours HH:MM-HH:MM` writes a quiet-hours preference; `--import-file PATH --import-kind fact|preference|project|feedback` imports plain text one memory per line (# comments and blank lines are skipped; ≤256 KiB, 200 lines, 2000 chars per line; no redaction); it is not yet a full interview or calendar/email import, and memory/goal writes are not atomic;
- TUI `/inbox`: list open / due-snoozed items (kind/status/title/why_now); supports `/inbox resolve <id> <approved|rejected|answered|dismissed> [note]` and `/inbox snooze <id> <until_ms>` with the same parser/applier as the CLI;
- Agent Card v0: `1agent agent-card [--json]` prints schema/version/platform/capability statuses/limits; `GET /v0/agent-card` and `GET /.well-known/agent-card.json` serve the same payload as public read-only HTTP routes. Platform-constrained capabilities come from the platform registry, and unsupported/untested capabilities must not be marked verified. `--generate-key [--private-out PATH --public-out PATH]` creates an Ed25519 key pair (prefer 0600 files; without `--private-out` it prints the private key with a warning), `--sign-key-file PATH` signs from a 0600 file (`--sign-key` remains but exposes the key in argv), and `--verify PATH --trusted-key HEX` verifies it; tampered, wrong-key and unsigned cards are rejected. `1agent card-trust add|revoke|list|verify` maintains local trust anchors; verification requires the card's embedded key to be trusted and not revoked. This is a local trust statement, not A2A registry/MD discovery;
- Tool search: `1agent tools --query TEXT [--json]` ranks tools deterministically by name/prefix/description/parameter names; `1agent run --tool-query TEXT` limits the model-visible tools to the top 8 plus always-on (`read_file`, `write_file`, `edit_file`, `terminal_exec`) and journals the selection; without a query the full list is used;
- CUA policy: `1agent cua-check` evaluates argument provenance (plan/trusted/untrusted), risk, reversibility and the planned branch into allow/require_approval/deny; high risk + untrusted → deny; the planner must not see untrusted content; an unplanned branch → deny. Policy only; no browser execution;
- Cascade containment: `1agent cascade-check --root NODE --edge PARENT>CHILD --contain NODE` computes affected/contained/propagated, containment_rate_bps, cascade_depth and blast_radius, then gates them against a policy. Deterministic simulation, not a live multi-agent run;
- Budget policy: `1agent budget-check` enforces hard limits on cost/tokens/tool calls (exceeded fails, 80% warns); `--verified-outcomes` reports cost per verified outcome; an exceeded budget exits non-zero. `1agent run --max-cost-micros/--max-tokens` accumulates provider usage at runtime and stops with `BudgetExhausted` when either limit is exceeded;
- OTel GenAI projection: `1agent otel --journal PATH [--json]` projects deterministic spans (invoke_agent / chat / execute_tool) with provider/model/token/cost attributes; it is a JSON projection, not a full OTel SDK/exporter;
- Chat adapter contract: `ChatMessage → TaskRequest`; `ChatReply` gives the conclusion, one trust line (evidence/unknowns), `needs_approval` and `inbox_open` without schema field names; `1agent chat-bridge task --message PATH --workspace DIR --out PATH`, `chat-bridge reply --result PATH --thread-id ... --out PATH` and `chat-bridge ingest --inbox DIR --workspace DIR --out DIR` provide a file-based bridge (message→task, run result→reply, directory spool); ingest moves valid messages to processed/, invalid ones to failed/ and exits non-zero when any failed; an existing `<stem>.task.json` is left untouched and the message is moved to processed/ as skipped; `--state PATH` records message SHA-256 values so the same content under a new name counts as a duplicate and is skipped; `--lock PATH` uses create_new to prevent concurrent ingests and removes the lock on normal exit (a crash leaves it for manual cleanup); it is not a Slack/Telegram/WeChat connection;
- One-way presence notification: `1agent inbox-notify --store PATH --webhook URL [--format generic|slack|discord] [--state PATH]` posts open inbox items to a webhook; HTTPS (or loopback HTTP) only, no embedded credentials and no redirect following, 5s timeout, at most 50 items and 200 title chars; an empty queue sends nothing; `--state` stores a watermark so only queues with a newer item are re-sent (the full open queue is still posted); `--log PATH` appends one JSONL receipt per successful send (time/open/new_items/format, no URL); `--dry-run` reports the open/new counts without calling the webhook or advancing the watermark; it does not read replies or change item state. The existing `notify` command sends one operator message;
- Team role policy: `1agent team-check` evaluates owner/approver/member/viewer against read/write/approve/manage and a scope; empty scopes mean all; unknown actors, disallowed actions and ungranted scopes are denied. `--audit-store PATH --actor-id NAME` appends the allow/deny verdict to the organization audit chain (denials are recorded too). Policy module, not an identity provider;
- Team handoff: `1agent handoff create --from ... --to ... --summary ... --evidence ... [--open-item ...]` appends to JSONL; at least one evidence reference is required and from≠to; only the addressee may accept, and a terminal handoff cannot transition again; `handoff cancel --id ... --actor ... [--reason ...]` lets from/to withdraw or decline while open; `handoff list [--open] [--to NAME]` is ordered by creation time and can filter by status/recipient. Local handoff, not live shared context;
- Durable idempotency records: `1agent idempotency record|replay --store PATH --key K --fingerprint F` appends one JSON line per key and syncs it before acknowledging; after a restart the same key+fingerprint replays the original response, while a different fingerprint conflicts and exits non-zero. A final line without a newline is treated as an interrupted append and ignored. This is a primitive for external writes, not yet wired into every tool;
- Out-of-band watchdog: `daemon --heartbeat PATH` atomically writes `{pid, updated_at_ms}` every loop; `1agent watchdog --heartbeat PATH --max-age-ms N [--json]` exits non-zero when the heartbeat is missing, stale or invalid. It only observes liveness; it does not restart the daemon or clear its lock;
- Daemon quiet hours: `daemon --quiet-hours HH:MM-HH:MM --utc-offset-minutes N` skips due jobs during the window. Jobs stay due and run after the window; the quiet-period sleep is `min(poll interval, milliseconds until the window ends)`, so the boundary is re-checked promptly, and the heartbeat still advances.
- False-alarm budget: `1agent alarm-budget --urgency N --delivered D --false-alarms F [--max-delivered N] [--max-false-alarm-bps N] [--critical-urgency N] [--json]`; when the delivery budget is exhausted or the false-alarm rate is too high, non-critical alerts move to a digest while critical alerts still deliver. Policy function only; it does not send or persist alerts;
- Escalation ladder: `1agent escalate --created-ms ... [--now-ms ...] --step 0=inbox --step 900000=notify --step 3600000=urgent [--resolved] [--json]`; selects the highest reached rung, never escalates a resolved item, requires strictly increasing thresholds, and only computes the plan. `daemon --escalate-inbox-store PATH --escalate-step ... --escalate-webhook URL --escalate-state PATH [--escalate-format ...]` projects on every tick and delivers only newly reached `notify`/`urgent` rungs; quiet hours suppress delivery. `1agent inbox escalate --step ... [--now-ms ...] [--json]` projects a level for every open or due-snoozed item from its `created_ms`; terminal items are excluded. With `--webhook URL --state PATH`, only newly reached `notify`/`urgent` rungs (higher than the highest rung recorded for that item) are delivered in one batch; `--dry-run` reports without sending. The state advances only after a successful send, so a crash in between can duplicate one batch. `1agent digest --escalation-step ...` adds level counts to the daily digest, including the `digest` rung that is not webhook-delivered;
- Trust disclosure manifest v0: `1agent trust-center [--evaluation PATH ...] [--verification-store PATH] [--security-contact ...] [--incident-url ...] [--out PATH] [--markdown-out PATH] [--json]` derives capability counts, limits and the data policy from the Agent Card and accepts caller-supplied evaluation disclosures; `--markdown-out` writes a human-readable disclosure alongside the JSON; `--verification-store` adds the local all-time verified/hacked/rejected/bps summary. Audit is fixed to `not_audited`/no external audit; it does not invent SOC2/ISO/EU certification. `1agent eval --suite ... --disclosure-out PATH` emits a compatible evaluation disclosure with the report SHA-256;
- Unit economics: `1agent unit-economics --revenue-micros N --variable-cost-micros N [--fixed-cost-micros N] [--outcomes N] [--json]` reports contribution/net, gross margin bps, revenue/cost per outcome and break-even outcomes. The caller supplies prices and costs; no pricing policy is built in;
- Organization audit index: `1agent org-audit append --actor ... --action ... --scope ... --decision allow|deny` appends a JSON line and syncs it; each entry commits to its predecessor with a SHA-256 hash chain. `org-audit verify` detects edited fields, reordering and broken links and exits non-zero. `generate-key` creates an Ed25519 key file, `seal --key ... --out ...` signs the entry count and head hash, `verify --seal ... --public-key ...` checks both chain and seal, and `list --limit N` shows the tail entries. It is a local index, not a remote transparency log, and it does not replace an IdP;
- MCP OAuth claims policy: `1agent mcp-token-check --claims PATH --audience ... --scope ... [--max-ttl-ms ... --skew-ms ... --now-ms ...]` enforces audience, expiry/not-before, maximum TTL and exact scope. The caller must verify the token signature first; this command does not verify signatures or store the token.

### 2.4 Local memory (available, RC)

- Working / Episodic / Semantic / Procedural layers;
- scope, source, taint, TTL and valid_until;
- recall, soft forget and hard forget;
- encrypted payloads (per-record DEK + per-scope KEK);
- UserModel (preferences/boundaries/autonomy/voice/memory consent; inspect with `1agent user-model`);
- memory bodies are not written into the journal; only metadata/summaries are recorded.

### 2.5 Skills (available, RC)

- SKILL.md, versions, index and scanning;
- static held-out evaluation;
- runtime canary, promotion and rollback (module/tests; Router real-time observation wiring still being completed);
- candidate and demonstration capture (demonstration data kept as hashes);
- malicious or over-privileged skills rejected;
- `1agent skill scan --path SKILL.md [--json]`: preflight frontmatter, control characters, key/secret markers and instruction-override markers without registering or executing the skill;
- `1agent skill verify --bundle PATH --sha256 HEX [--public-key PATH] [--json]`: shares `verify_bundle` with import to check SHA-256, schema, signature, duplicates, manifest hashes and the registry scan; it does not import.

### 2.6 Long-lived capabilities (available / partial)

| Capability | Status | Current boundary |
|---|---|---|
| Goal / Commitment | Available (RC) | State machine, persistence, Commitment→Ticket link; dependencies/evidence/recurrence still being completed |
| Attention | Basic | Deterministic deadline reminders, budget, dedupe, snooze; full candidates/digest R&D |
| PersonalContextGraph | MVP | Read-only reconstruction from goals/commitments/tickets/threads/memory; cross-device/large-scale stress not done |
| DelegationProfile | Available (module) | observe/suggest/draft/ask/act-and-report; runtime wiring and real action domains limited |
| Self-summary / evolution | Basic | Reflection/Lesson, EvolutionProposal, Ledger, human approval/rollback; automatic candidates R&D |

### 2.7 Model routing (available, RC)

- deterministic provider and OpenAI-compatible provider;
- capability hard filters, quality/balanced/latency/cost objectives;
- provider health degradation, cooldown and shadow execution;
- cost/latency ledger;
- credentials referenced through environment variables or local configuration.

### 2.8 Audit and evidence (available, RC)

- append-only hash-chained journal;
- replay, audit receipt, query and export;
- ActionLog;
- tool call/observation pairing;
- exit 0 is not automatically promoted to verified;
- Artifact existence, path constraints and SHA-256 verification.

### 2.9 Local service and integration (available, RC subset)

- `1agent serve`: stdio host protocol;
- `1agent surface serve`: loopback HTTP Surface API with a token;
- partial thread, ticket, memory, skill, audit, ActionLog, goal/commitment/delegation/evolution APIs;
- not a multi-tenant SaaS, not a public internet service.

### 2.10 Productization (available, RC)

- one runtime home: `$ONEAGENT_HOME` / `$HOME/.1a`;
- `init` / `doctor` / `export` / `restore` / `upgrade` / `uninstall`;
- key escrow / restore;
- Linux release archive, SHA256SUMS, Linux provenance and installer;
- platform capability registry: `1agent platform [--json]` reports `verified` / `degraded` / `unsupported` / `untested`; macOS/Windows are `untested` and are not claimed as supported;
- MIT license and third-party notices.

## 3. Explicitly not implemented

| Capability | Status | Notes |
|---|---|---|
| Browser / computer-use | **Partial** | `browser_read` renders with headless Chrome and extracts text; clicking, typing, screenshots and visual understanding are not implemented |
| Generic subagents | **Partial** | `spawn_agent` runs bounded child agents (depth 2, isolated journal/workspace); Master→Worker remains a separate path |
| MCP client | **Partial** | stdio and streamable HTTP (JSON/SSE responses, `Mcp-Session-Id`, bearer token from an environment variable only, per-server `timeout_seconds`) discovery and calls; OAuth claims validation is available; issuer discovery, token exchange and a tool marketplace are not implemented |
| Always-on daemon/Routine/Trigger | **Partial** | Interval and file triggers, restart catch-up, `--routine-skill`, `schedule health`; failed/timed-out runs can write alerts and webhook them; escalation can be projected and delivered manually (`inbox escalate`) or by the daemon (`--escalate-*`); `digest`-rung delivery, stop-after-ack and per-channel ladders are not implemented, and cross-restart deduplication is not wired into the daemon |
| Full Attention/Digest | **Partial** | `digest` summarizes recent runs, schedules, health, alerts, open inbox items, open handoffs for a named recipient and the local verification summary; `--escalation-step` adds escalation level counts; `--notify-webhook` posts the same text digest one-way; the maintenance loop is not implemented |
| Automatic evolution/skill marketplace | **Partial** | Human approval/rollback plus SHA-256/Ed25519 skill bundles with signature verification; a remote marketplace index is not implemented |
| Voice/image understanding | **Not implemented** | `read_pdf` extracts PDF text only; voice, images and scanned-document OCR are not implemented |
| Multi-tenant/organization audit | **Not implemented** | Single-user, local-first today |
| Android/iOS app | **Planned** | With the Bot/mobile work |
| macOS support | **Planned** | Linux is the only validated platform |
| Windows support | **Internal build, not published** | Untested, no download |

## 4. Typical use cases

### A. Consequential decisions

```bash
1agent run --task "Should we migrate to the new architecture now or next quarter?"
```

The result should include a conclusion, facts, assumptions, unknowns, conflicts, flip conditions and supporting evidence.

### B. Reading code and documents

```bash
1agent run --read-root ~/project \
  --task "Trace the authentication path and list the three riskiest entry points"
```

Available tools include `read_file`, `read_pdf`, `search_files`, `fetch_url`, `web_search`, `browser_read`, plus `write_file`, `edit_file` and real PTY terminal tools (writes are on by default; `--read-only` disables them).

### C. Interactive investigation

```bash
1agent tui
```

Use `/frame`, `/evidence`, `/audit` and `/report` to organize the question and the evidence.

### D. Controlled execution (requires Workshop)

The local CLI/TUI allows file writes and terminal execution by default. With Skynet Workshop, terminal tools use persistent PTY sessions and can execute, read and destroy them. Workshop is still in R&D/integration and is not a stable production dependency.

## 5. Boundaries of this version

- **Not production-stable**: R1 release candidate; interfaces and behavior may change.
- **Not a complete Super-Agent**: computer-use actions, voice/image understanding, automatic watchdogs and mobile are unfinished.
- **Not Bot**: no persona, Web/IM/Voice UX or Companion Contract.
- **Not multi-tenant**: no organization IdP, tenant isolation or third-party audit.
- **Not an “operate any website” tool**: it can render and read pages, not click or take screenshots.
- **Not an authorization bypass**: local writes/execution are on by default; network needs `--allow-network`, and high-risk actions still pass Authority/Policy and approvals.

## 6. Related documentation

- [Design Philosophy](design-philosophy.en.md)
- [Manual](manual.en.md)
- [Ecosystem & Platforms](ecosystem.en.md)
- [Status & Roadmap](status.en.md)
- [Release Notes](release/RELEASE_NOTES.en.md)

---

[Back to README](../README.en.md)
