# 1Agent Threat Model (v0.97)

- Status: **local threat model, self-reviewed; no external audit**
- Date: 2026-11-12
- Scope: local CLI/TUI, daemon, worker, MCP/HTTP adapters, skills, memory,
  journals, tools, CUA policy
- Out of scope: cloud control plane, mobile apps, marketplace operations

This document is a P0/P1 threat inventory, not a certification. A missing
entry does not mean the threat does not exist; it means it was not reviewed.

## 1. Assets

- User files inside configured read roots; write targets.
- Provider credentials, MCP tokens, signing keys, escrow recovery keys.
- Run journals, audit receipts, verification records, memory payloads.
- User Model, goals/commitments, session preferences.
- Installed skills and their manifests.
- Terminal/browser sessions, MCP server connections.
- Cost/token budgets and the ability to stop a run.

## 2. Trust boundaries

1. **User ↔ CLI/TUI**: the user is trusted to configure; input is still
   sanitized for terminal control characters and path traversal.
2. **Agent ↔ model provider**: the provider is remote and untrusted output.
3. **Agent ↔ tools**: files, network, processes, browser and MCP servers are
   untrusted or partially trusted.
4. **Agent ↔ skills**: a skill is untrusted supply-chain content until
   scanned, evaluated and activated.
5. **Run journal ↔ disk**: local file integrity and permissions.
6. **Daemon ↔ inbox/webhooks/MCP**: remote inputs and at-least-once delivery.
7. **Host ↔ platform**: Windows/macOS permission and secret-storage
   differences are not yet verified.

## 3. P0 threats

| # | Threat | Mitigation in tree | Residual gap |
|---|---|---|---|
| P0-1 | Prompt injection through tool output, screen text, attachments or MCP content steers an action | Explicit untrusted blocks, planner quarantine, argument provenance, approval for untrusted/irreversible actions, CUA branch limits | A model may still follow injected text in prose; no guarantee |
| P0-2 | Secret leaks to provider, logs, journal, webhook or skill draft | `AGENT_API_KEY` env-only, no argv key unless explicit, secret-name redaction in skill capture, no token echo in errors, HTTPS/loopback rules | Heuristic redaction; not full DLP |
| P0-3 | Unauthorized write/execute | `PermissionSet`, write tools hidden without `allow_write`, execution gate, approval, dry-run/repair receipts | Same-UID local processes remain unrestricted unless capability policy is configured |
| P0-4 | SSRF / private-network access | `fetch_url`/`web_search` reject private, loopback, link-local and metadata targets; redirects revalidated; no redirects for webhook/MCP clients | DNS rebinding and platform-specific address classes are not exhaustively tested |
| P0-5 | Approval/authority bypass | Authority profiles, actor checks, HMAC approval envelopes, ticket state machine, audit chain | Approval UI is local; no external identity provider |
| P0-6 | Journal tampering or hidden tool effects | Contiguous seq, run-id checks, duplicate/unpaired tool-call audit, replay, checkpoint deltas, org audit hash chain | Run journals have no cryptographic hash chain or external anchor |
| P0-7 | MCP token theft / token passthrough | 0600 token file, audience/TTL/scope claims, token exchange, no redirects, session reuse checks | Token file TOCTOU was fixed by review; host key storage varies |
| P0-8 | Malicious skill or bundle | Scan, manifest validation, signature verification, immutable versions, held-out eval, canary/rollback | Static marker suite is not malware sandboxing; no marketplace reputation |
| P0-9 | CUA screen injection / branch steering / irreversible click | Single-shot plan, freeze/authorize, provenance, red-team fixtures, human takeover policy | No browser executor exists yet; execution safety is unproven |
| P0-10 | Cost/token/resource runaway | Hard budgets, step/tool/provider limits, timeout, response size caps, parallel read-only cap by batch size | No global concurrency semaphore; streaming usage may undercount if provider omits it |

## 4. P1 threats

| # | Threat | Mitigation | Gap |
|---|---|---|---|
| P1-1 | Cross-bot/team memory or inbox scope escape | Scope checks in memory, inbox and team tests | No multi-tenant deployment test |
| P1-2 | Webhook/MCP SSRF via redirects or embedded credentials | HTTPS/loopback rules, no redirects, reject credentials | Custom allowlists are operator-owned |
| P1-3 | Watchdog spoofing or false alarm | Heartbeat age + escalation ladder + intervention log | PID is not verified as the daemon; age alone is not liveness |
| P1-4 | Daemon replay/duplicate webhook delivery | Watermark, at-least-once semantics documented | Crash window can duplicate once |
| P1-5 | Local same-UID attacker reads state | Private 0600/0700 files where implemented; capability policy exists | Default is "not configured"; same-UID clients unrestricted |
| P1-6 | Journal/log retention leaks sensitive content | Retention script, local stores | No encryption at rest for run journals |
| P1-7 | Platform secret storage and ACLs | Linux 0600/0700; Windows ACL/DPAPI planned | Windows/macOS not verified on real hosts |
| P1-8 | Supply-chain dependency compromise | Locked Cargo.lock, offline builds, attribution gate | No vendoring/SBOM verification in CI |
| P1-9 | Model misreports completion | Verification gate, hidden criteria, artifact SHA-256, model self-report not accepted | Verification quality depends on evaluator and hidden tests |

## 5. Security invariants checked by tests

- `cargo test cua --lib`, `tests/cua_check_cli.rs`: provenance, planner
  quarantine, branch steering, single-shot plan.
- `tests/red_team.rs`: memory scope isolation, ticket authority, approval
  bypass, side-effect idempotency.
- `tests/hack_verifiable.rs`: self-verification and hidden-test failures.
- `tests/mcp_token_exchange_cli.rs`, `tests/mcp_auth` unit tests: audience,
  TTL, redirect and token file behavior.
- `tests/recovery_soak.rs`: crash resume does not replay a completed side
  effect; unpaired/checkpoint-less journals fail closed.
- `scripts/verify.sh` forbids platform branching outside
  `tui_terminal.rs`; `scripts/check-attribution.sh` forbids AI bylines.

## 6. P0/P1 open items

1. **No CUA executor**: the policy is ready, but clicking, typing and
   screenshotting do not exist. When an executor lands it must call
   `CuaPlan::authorize` before every action.
2. **No cryptographic run-journal chain**: org audit is hash-chained, run
   journals are not. A local writer can rewrite a journal without a
   cryptographic anchor.
3. **Same-UID isolation is not configured by default**; the doctor reports
   this. A local process running as the same user can read state.
4. **Windows/macOS secret storage and ACLs are untested**; Linux is the only
   verified platform.
5. **No external security/compliance audit**; SOC 2 / ISO 42001 / EU AI Act
   mapping is not done.
6. **Skill scanning is heuristic**, not a malware sandbox.
7. **Watchdog is advisory**: it does not signal or restart the daemon.

## 7. Review record

- 2026-11-12: local review; independent code-review agents previously found
  and confirmed fixes for secret redaction, HTTP disclosure wording and
  token-exchange boundaries. Those reviews are code-level, not a threat-model
  audit.
- Next: external audit (SA100-68) and a real cross-platform smoke.
