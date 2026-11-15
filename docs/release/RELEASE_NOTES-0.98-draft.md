# 1Agent 0.98.0-dev Draft Release Notes

Status: **developer draft; not a release announcement**  
Date: 2026-11-15  
Baseline: `0.98-dev`, package version `0.98.0-dev`

## What changed

### Onboarding and CLI

- `1agent setup` guided first run: military-green 1AGENT ASCII welcome,
  arrow-key provider/model/permissions/workspace menus, masked key,
  `/v1/models` probe with a `not advertised` warning for the current model,
  30s smoke, doctor summary. Idempotent, resumable, `--check`,
  `--non-interactive`, `--api-key-stdin`; `--api-key` argv is rejected.
- `1agent status` human/JSON projection.
- 13 public commands; advanced commands behind `help advanced` / `help --all`.
- `model list/show/use/test`, `permissions show/explain/set`.
- Config v2: default provider, workspace, permission preset, 0600 key file.
- `run`/`tui` load `$ONEAGENT_HOME/config.json` by default.
- Shell completions from the live clap tree; static man page.

### Browser / computer-use

- Per-task Chrome profile, CDP driver, navigate/read/click/type/press/scroll,
  screenshots, screencast JPEG frames + index.
- Local CONNECT/HTTP egress proxy: allowlist, DNS pinning, private/metadata
  address rejection.
- `secret://` origin-bound credential broker; values never enter
  prompt/journal/metadata; plaintext in password fields is refused.
- `browser` agent tool (write-gated) and `browser probe/open/record` CLI.
- Headed human takeover; Chrome sandbox preserved; no `--no-sandbox`.
- VAC-style action cards are the mandatory path for buttons, submits,
  cross-origin/download links, password entry and Enter. Cards persist at
  `$ONEAGENT_HOME/browser/approvals.json` (0600), bind the exact DOM
  descriptor/session/origin, expire after 120s and are consumed once.
  Humans use `browser approvals|approve|deny`; a fake page confirmation
  cannot authorize an action.

### MCP

- Dual-era client: modern 2026-07-28 `server/discover`/`_meta`/headers plus
  legacy `initialize` fallback.
- OAuth 2.1 authorization-code + PKCE S256 with loopback callback, RFC 8707
  `resource`, 0600 token file, refresh.
- Tool-definition canonical SHA-256 pinning (`pin_file`), fail closed.
- Argument JSON Schema validation (required/type/additionalProperties subset).
- HTTP SSRF guard via public-IP resolution.
- Server annotations cannot lower local permissions.
- `x-mcp-header` arguments are mirrored as `Mcp-Param-*` (raw or Base64
  sentinel) and validated against the body.
- CIMD client metadata document URL used as `client_id`; RFC 9207 `iss`
  validated; Tasks extension opt-in with bounded polling; MRTR
  `input_required` fails closed without echoing `requestState`.
- Real-server evidence: modern/legacy stdio conformance configs,
  skynet-prompt stdio, and modern HTTP `header_echo` (`tests/mcp_real_servers.rs`).

### Skills

- `SKILL.md` directory validator (symlink/path/size/script/external-reference
  checks).
- Execution eval (`skill eval-runtime`) with held-out markers; output hashed,
  not stored.
- Activation requires a passing eval report or explicit `--force`.
- Persistent canary state; persistent candidate/demonstration store.
- Blind A/B comparator and deterministic held-out split with Wilson 95% lift.
- Active-version lockfile (`skill lock`/`verify-lock`) with exact drift detection.
- Opt-in `skill run-script` sandbox via bwrap: no network, no host home,
  read-only runtime/skill dir, tmpfs `/tmp`, dropped capabilities, timeout and
  output caps; bwrap absence fails closed.
- Existing Ed25519 bundle signing and rollback preserved.
- Malicious-skill corpus: 8 adversarial fixtures (frontmatter injection,
  hidden zero-width/bidi instructions, secret-like tokens, oversized/nested
  bundles, asset symlink, mutable external URL warning, script warning).
  `scan_text` rejects invisible format characters; ZWJ is left alone for emoji.

### Daemon

- Durable `ScheduleRunLedger`: claim before spawn, at-most-once, reconcile
  interrupted on restart, duplicate-slot skip.
- Deterministic jitter; `--missed-run-skip-after-seconds`.
- Hardened systemd user unit generator; no automatic linger.
- `sd_notify` READY/WATCHDOG (filesystem socket).
- 100-slot claim/reconcile matrix test; 100-cycle real child-process crash
  matrix (`tests/ledger_crash.rs`).

## Verification run

```text
cargo fmt --check                                  ✅
cargo clippy --offline --all-targets -- -D warnings ✅
cargo test --offline -j 2                          ✅
./scripts/verify.sh                                ✅
  core eval 8/8, hard-gate failures 0, rubric 6/6
  claims 251 (227 verified / 16 partial / 8 no evidence)
real Chrome E2E                                    ✅ navigate/click/type/screenshot/record
browser action cards + secret entry + fake confirm ✅ live Chrome, single-use approval
100-cycle process-kill ledger matrix               ✅ 0 duplicate claims
skill blind compare + lockfile + bwrap run-script  ✅ library + CLI tests
skill malicious corpus                             ✅ 8/8 adversarial fixtures
setup wizard PTY E2E                               ✅ fresh/unadvertised/masked/idempotent/cancel/narrow
W1 real repository maintenance                     ✅ agent patch + independent tests/audit/replay
per-gate evidence                                  ✅ docs/evidence/0.98-*
MCP legacy + modern + pin + schema + Tasks/MRTR    ✅
MCP real stdio/modern HTTP servers                 ✅ Codex conformance + skynet-prompt
1h M1 (deepseek-flash)                             ✅ rerun3 239/240 = 99.58%, replay 100%, 0 crashes
```

## Known gaps (not release-ready)

1. MCP has local real-server evidence (Codex conformance modern/legacy/HTTP,
   skynet-prompt) but no remote hosted server or OAuth authorization-server
   evidence; in-task input (`tasks/update`) is not implemented and fails
   closed.
2. Browser action cards are a human-consent/audit control, not a hard boundary
   against a model that also has terminal-write permission. Run W2 without
   terminal write access. bwrap/Landlock/seccomp is deferred; Chrome sandbox is
   preserved. Secret text-field masking is best-effort (`-webkit-text-security`),
   not a page-JS boundary.
3. Skill blind comparator/held-out split are library primitives, not yet a
   mandatory activation gate. `skill run-script` is explicit and sandboxed;
   skill scripts are never auto-executed.
4. W1 is complete (`docs/evidence/0.98-w1/`); W2/W3 need the owner's real
   systems/accounts; no consolidated adaptive red-team report yet.
5. 1h M1 is green: rerun3 239/240 = 99.58%, completed replay 100%, 0 crashes
   (`docs/evidence/m1-1h-2026-11-15-rerun3/`). The 8h soak and 100-question
   rerun are not done. The M1 binary predates the TUI interaction changes;
   `benchmark-run` does not use the TUI.
6. Packaging/SBOM/attestations/release provenance and owner publication
   approval are pending; the tree is uncommitted.

## Release rule

Do not publish 0.98.0 until the must-pass gates in
`docs/0.98-development-plan.md` and `docs/0.98-progress.md` are green, and the
owner explicitly approves push/tag/package publication.
