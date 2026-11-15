# 0.98 Red-Team Evidence (Working Draft)

Date: 2026-11-15  
Status: **partial** — policy-level fixtures and live Chrome tests exist; see `REPORT.md` (ASR 0/14, utility 6/6, false-block 0/6, wide intervals). The adaptive/model-level campaign is not complete.

This directory records the current security fixtures and the honest gaps. It is
not a claim of external audit or universal prompt-injection immunity.

## Test mapping

| Threat | Control | Evidence | Status |
|---|---|---|---|
| Browser SSRF to loopback/private/metadata | egress proxy + `is_global_ip` | `src/browser/egress.rs` unit tests | ✅ |
| DNS rebinding / redirect-to-private | resolve → validate → pin; every proxy connection revalidated | egress unit tests + code path | ◐ (unit only; no adaptive browser test) |
| Credential exfiltration | `secret://` origin binding; model never sees value | `src/browser/credentials.rs` tests; `tests/secret_cli.rs` | ✅ |
| Model-generated action approval | persistent VAC action card bound to DOM descriptor/session/origin | `src/browser/policy.rs` tests; `tests/browser_tool_approval.rs`; `tests/browser_approvals_cli.rs` | ✅ |
| Prompt injection in page content | page text never authorizes; fake "Approved" button/cross-origin/download links still require a card | `tests/browser_tool_approval.rs` | ◐ (policy-level; no adaptive model corpus) |
| MCP tool poisoning / rug pull | canonical full-definition hash pinning, fail closed | `src/tools/mcp.rs` pin tests | ✅ |
| MCP header/body desync | modern `Mcp-Method`/`Mcp-Name` sent; server-side validation required | modern fake server test checks headers | ◐ (client-side only) |
| MCP argument confusion | JSON Schema subset validation | `mcp_argument_schema_validation_fails_closed` | ✅ |
| MCP HTTP SSRF | public-IP resolution before reqwest | `http_transport` + existing web resolver | ◐ (no dedicated MCP SSRF test yet) |
| OAuth token passthrough | audience/resource binding; CIMD client id; RFC 9207 issuer validation | PKCE/CIMD/issuer tests | ◐ (no live AS test) |
| Malicious skill symlink/path escape | validator rejects symlinks/oversize/injection markers | `src/skill_validate.rs`, `tests/skill_validate_cli.rs` | ✅ |
| Skill activation without eval | activation gate or explicit `--force` | `tests/skill_validate_cli.rs` | ✅ |
| Scheduled duplicate side effects | run ledger claim/reconcile, at-most-once | `src/schedule_ledger.rs` tests | ✅ |
| Daemon crash mid-run | interrupted marker, no auto-retry | `tests/ledger_crash.rs` 100 child-process crashes | ✅ |
| KILL switch / read-only | existing runtime gates | existing 0.97 tests | ✅ (regression) |

## Known gaps

1. No adaptive browser prompt-injection corpus (AgentDojo/BrowserART-style) with ASR/utility/false-block reporting. Policy-level fixtures show fake confirmation cannot authorize, but model-level attack success is untested.
2. Action cards are a consent/audit control; a model with terminal-write permission can invoke `browser approve`. W2 must run without terminal write.
3. No bwrap/Landlock/seccomp browser wrapper; Chrome's own sandbox is preserved and `--no-sandbox` is never used. No screenshot masking for secret text fields.
4. MCP MRTR/Tasks tests cover fail-closed input handling and URL origin redaction; no live AS/Tasks TTL abuse or third-party server evidence.
5. No malicious-skill corpus beyond symlink/path/injection/script fixtures.
6. No consolidated OWASP Agentic Top 10 / Agentic Skills Top 10 / MCP Top 10 report with confidence intervals.
7. No independent third-party review for 0.98.

## Gate

0.98 release gate is **not met** until the adaptive corpus (gap 1), live MCP
server evidence (gap 4), consolidated report (gap 6) and independent review
(gap 7) are closed or explicitly accepted by the owner in release notes.
