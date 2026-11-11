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
- unauthorized actions are rejected before tool execution.

### 2.4 Local memory (available, RC)

- Working / Episodic / Semantic / Procedural layers;
- scope, source, taint, TTL and valid_until;
- recall, soft forget and hard forget;
- encrypted payloads (per-record DEK + per-scope KEK);
- UserModel;
- memory bodies are not written into the journal; only metadata/summaries are recorded.

### 2.5 Skills (available, RC)

- SKILL.md, versions, index and scanning;
- static held-out evaluation;
- runtime canary, promotion and rollback (module/tests; Router real-time observation wiring still being completed);
- candidate and demonstration capture (demonstration data kept as hashes);
- malicious or over-privileged skills rejected.

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
- MIT license and third-party notices.

## 3. Explicitly not implemented

| Capability | Status | Notes |
|---|---|---|
| Browser / computer-use | **Partial** | `browser_read` renders with headless Chrome and extracts text; clicking, typing, screenshots and visual understanding are not implemented |
| Generic subagents | **Partial** | `spawn_agent` runs bounded child agents (depth 2, isolated journal/workspace); Master→Worker remains a separate path |
| MCP client | **Partial** | stdio MCP discovery and calls; HTTP/SSE, OAuth and a tool marketplace are not implemented |
| Always-on daemon/Routine/Trigger | **Partial** | Interval and file triggers, restart catch-up, `--routine-skill`, `schedule health`; failed/timed-out runs can write alerts and webhook them; escalation and cross-restart deduplication are not implemented |
| Full Attention/Digest | **Partial** | `digest` summarizes recent runs, schedules, health and alert counts; the maintenance loop and escalation ladder are not implemented |
| Automatic evolution/skill marketplace | **Partial** | Human approval/rollback plus SHA-256 skill bundles; signatures and a remote marketplace index are not implemented |
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
