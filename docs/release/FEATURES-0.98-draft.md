# 1Agent 0.98 Draft Feature Matrix

Status: **draft for development gates; not a release announcement**  
Baseline: `0.98-dev`  
Date: 2026-11-15

Legend: ✅ implemented and tested · ◐ partial · 🧪 experimental · ✖ not implemented

> This matrix is intentionally stricter than a marketing page. “✅” means the
> repository has tests/evidence for the listed path, not that the feature has
> passed an external audit.

## 1. Onboarding and CLI

| Feature | Status | Entry | Notes |
|---|---|---|---|
| Guided setup | ✅ | `1agent setup` | menu-driven provider/model/permissions/workspace; model probe; smoke; idempotent, resumable |
| Non-interactive setup | ✅ | `setup --non-interactive` | no TTY hangs; explicit missing values exit 2 |
| `setup --check` | ✅ | `setup --check` | read-only; no writes/prompts |
| Provider probe | ✅ | setup / `model test` | `/models`, 5s timeout, redirects off, loopback HTTP only |
| 0600 key store | ✅ | `~/.1a/keys/provider-<id>.key` | stdin input; `--api-key` argv rejected |
| Permission presets | ✅ | `permissions show/explain/set` | read-only / safe / full-local; full-local still confirms irreversible actions |
| Status | ✅ | `1agent status --json` | version/platform/provider/permissions/home/last run |
| Public/advanced IA | ✅ | `help`, `help advanced`, `help --all` | 13 public commands; advanced grouped |
| Completions | ✅ | `completions <shell>` | live clap tree via `clap_complete` |
| Man page | ◐ | `man` | static roff draft; not yet clap_mangen |
| Default config | ✅ | `$ONEAGENT_HOME/config.json` | `--config` still overrides |
| Interactive wizard | ✅ | `1agent setup` on a TTY | military-green ASCII welcome; arrow-key menus; masked key; unadvertised-model warning; PTY E2E; manual 5-user timing not done |

## 2. Computer-use / browser

| Feature | Status | Entry | Notes |
|---|---|---|---|
| Per-task Chrome profile | ✅ | `browser` tool / `browser probe` | ephemeral 0700 profile; no user profile |
| CDP driver | ✅ | internal | Page/Runtime/Input/Accessibility/Target/Browser |
| Navigate/read | ✅ | `browser probe` | title, visible text, AX tree |
| Click/type/press | ✅ | `browser` tool / library | real Chrome E2E |
| Screenshot | ✅ | `browser probe --screenshot` | PNG |
| Screencast recording | ✅ | `browser record` | JPEG frames + `index.json`; no ffmpeg |
| Per-task egress proxy | ✅ | internal | allowlist, DNS pinning, private/metadata rejection |
| Credential broker | ✅ | `secret set/list/remove`, `browser` tool | `secret://`, exact origin, never logged |
| Human takeover | ✅ | `browser open` | headed Chrome; Ctrl+C/seconds; no concurrent agent control lease |
| Action cards | ✅ | `browser approvals/approve/deny` | persistent 0600 card, descriptor/session/expiry binding, single-use; live Chrome test; secret fields masked for screenshots |
| Chrome sandbox | ✅ | launcher | never `--no-sandbox` |
| bwrap/Landlock/seccomp | ✖ | — | future hardening |
| Red-team suite | ◐ | tests | SSRF/private-IP + fake-confirmation/action-card fixtures; adaptive model-level suite pending |

## 3. MCP

| Feature | Status | Entry | Notes |
|---|---|---|---|
| stdio transport | ✅ | `mcp_config` | existing |
| Streamable HTTP | ✅ | `mcp_config` | existing |
| Modern 2026-07-28 | ✅ | internal | `server/discover`, `_meta`, `Mcp-Param-*` header mirroring, no session |
| Legacy 2025-11-25/2025-06-18/2024-11-05 | ✅ | internal | initialize fallback |
| OAuth token exchange | ✅ | `token_exchange` | RFC 8693 |
| OAuth authorization-code + PKCE | ✅ | `oauth` config | loopback callback, RFC 8707 resource, 0600 token file, refresh |
| CIMD client metadata documents | ✅ | `oauth.client_metadata_document` | HTTPS URL client_id, path/no-fragment/no-userinfo validation; document must be hosted by operator |
| Tool definition pinning | ✅ | `pin_file` | canonical SHA-256, fail closed |
| JSON Schema validation | ✅ | internal | required/type/additionalProperties (conservative subset) |
| HTTP SSRF guard | ✅ | internal | public-IP resolution |
| Annotations cannot lower permissions | ✅ | internal | all MCP tools write-gated |
| Tasks/MRTR | ◐ | `tasks: true` | bounded `tasks/get` polling; `input_required` fails closed, no auto-elicit; `tasks/update` not implemented |
| Real-server evidence | ◐ | `tests/mcp_real_servers.rs` | 3 stdio configs (modern/legacy/prompt) + modern HTTP header mirroring; no remote OAuth server |

## 4. Skills

| Feature | Status | Entry | Notes |
|---|---|---|---|
| SKILL.md validator | ✅ | `skill validate` | symlink/path/size/script/external-ref checks |
| Static marker eval | ✅ | `skill eval` | existing |
| Execution eval | ✅ | `skill eval-runtime` | model runner; output hashed, not stored |
| Activation gate | ✅ | `skill activate` | passing eval report or `--force` |
| Persistent canary | ✅ | `skill canary --state` | survives invocations |
| Rollback | ✅ | `skill canary --apply` / service | baseline restore |
| Ed25519 bundle signing | ✅ | `skill export/import/verify` | existing |
| Active-version lockfile | ✅ | `skill lock` / `verify-lock` | exact two-way active set, content hashes, drift fails |
| Persistent candidate store | ✅ | `SkillCandidateStore::open/persist` | 0600 JSON, hashes only |
| Blind comparator | ✅ | `skill compare --suite` | seeded A/B arms, output hashes only, Wilson 95% interval; nonzero exit unless significant |
| Script sandbox | ✅ | `skill run-script` | explicit bwrap run: no network, no host home, read-only runtime/skill dir, tmpfs /tmp, caps dropped, timeout/output caps; fail closed if bwrap missing |
| Malicious-skill corpus | ✅ | `tests/skill_malicious_corpus.rs` | 8 adversarial fixtures: frontmatter injection, hidden zero-width/bidi, secret-like token, oversize/nesting, asset symlink, external URL warning, script warning; adaptive model-generated corpus still pending |

## 5. Daemon

| Feature | Status | Entry | Notes |
|---|---|---|---|
| Durable run ledger | ✅ | `daemon --run-ledger` | claim/reconcile; at-most-once |
| Duplicate-slot skip | ✅ | daemon loop | claim before spawn |
| Interrupted recovery | ✅ | daemon startup | marks claimed/running interrupted; no auto-retry |
| Systemd user unit | ✅ | `daemon --print-unit/--write-unit` | hardened, no linger |
| sd_notify | ✅ | daemon loop | filesystem socket; abstract socket unsupported |
| Health/alerts/digest/escalation | ✅ | existing | unchanged |
| Deterministic jitter | ✅ | daemon loop | 0..5% interval, stable by job id |
| Missed-run skip | ✅ | `--missed-run-skip-after-seconds` | skip-if-late; default catches up the latest slot |
| 100-crash matrix | ✅ | `tests/ledger_crash.rs` | 100 child processes claim then `_exit(137)`; 0 duplicate claims |
| Auto resume/kill | ✖ | — | deliberately not implemented |

## 6. Workflows and red team

| Feature | Status | Notes |
|---|---|---|
| W1 repo maintenance | ✖ | needs a real issue + provider |
| W2 browser operations | ✖ | needs the owner's real backend/account |
| W3 scheduled digest | ✖ | needs GitHub + Feishu/Slack channels |
| Browser SSRF/private-IP fixtures | ✅ | unit/E2E |
| MCP pin/schema/header fixtures | ✅ | unit |
| Skill symlink/injection fixtures | ✅ | unit |
| Consolidated OWASP report | ✖ | pending |
| Independent review | ✖ | pending |

## 7. Release

| Feature | Status | Notes |
|---|---|---|
| Development plan/ADR/design | ✅ | docs |
| Progress ledger | ✅ | `docs/0.98-progress.md` |
| Feature matrix | ◐ | this draft |
| Manual/README update | ◐ | 0.98 setup/browser/MCP sections updated; final release pass pending |
| Package with completions/man | ◐ | `scripts/package.sh` generates both; packaging needs clean commit/tag |
| SBOM/attestations/signing | ✖ | pending |
| 1h M1 | ✅ | 239/240 = 99.58%, completed replay 100%, 0 crashes (`docs/evidence/m1-1h-2026-11-15-rerun3/`) |
| 8h soak / 100 questions | ✖ | pending |
| Public release | ✖ | owner approval required |

## 8. Session interaction (post-gate enhancement)

| Feature | Status | Entry | Notes |
|---|---|---|---|
| Session picker | ✅ | `←` on an empty composer | military-green modal; selection auto-previews; search/rename/archive/new; transcript restore |
| Follow-up context | ✅ | any new prompt | recent request, conclusion/status, claims, evidence, tool summaries; ≤12k chars |
| Running input queue | ✅ | `Enter` while running | auto-submit after the run; `↑` recalls the newest queued message |
| Pause / interrupt | ✅ | `Esc` / second `Esc` | UI-level pause (`⏸ 已暂停`), `Enter` resumes; second `Esc` or `Ctrl+C` interrupts; pause time excluded from UI timer |
| Checkpoint resume | ✅ | `Ctrl+R` | terminal journal truncated at the last checkpoint; no checkpoint fails visibly |
| Agent-分队 v1 | ✅ | `Ctrl+G`, `Tab` in picker | read-only child-process board; `spawn_agent` registry; live child-journal preview; no stop/attach/retry |
| Runtime-level pause / multi-session | ✖ | — | P2; parent pause does not pause a running child |

Design: `docs/0.98-session-interaction.md`. Evidence:
`docs/evidence/0.98-session-interaction/`.
