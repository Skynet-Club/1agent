# Release Notes — 0.96.0 Action Layer Preview

Date: 2026-11-10  
Status: **R1 public pre-release (release candidate), not production-stable**

GitHub pre-release: <https://github.com/Skynet-Club/1agent/releases/tag/v0.96.0>

[简体中文](https://github.com/Skynet-Club/1agent/blob/main/docs/release/RELEASE_NOTES.md) · **English**

## Development status and community

1Agent is still under rapid iteration. 0.96.0 is an R1 release candidate, not a
finished or production-stable product; interfaces, configuration and data
formats may still change.

We welcome developers, testers, researchers, security specialists, product and
design partners to test the Linux build, report issues, submit pull requests,
join red-team exercises and validate real workflows. Join **Skynet-Club** to help
build 1Agent and its open ecosystem: <https://skynet.club>.

## Highlights

### 1. From answering to acting

- Local `run` / `tui` now enable file writes and terminal execution by default;
  `--read-only` disables them, and `--allow-write` remains accepted for
  compatibility.
- Added `write_file` / `edit_file`: allowed-root containment, symlink rejection,
  overwrite protection, atomic replacement and bounded content.
- Added real PTY terminal tools (`terminal_create/write/output/exec/destroy`)
  backed by Skynet Workshop. They are hidden from the model and denied by the
  registry without write authority. Verified on Linux x86_64.
- Added `fetch_url`: public HTTPS only, private/loopback address rejection,
  per-hop redirect validation, a 2 MiB cap and HTML text extraction.
- Added `web_search` through Bing RSS: no key, bounded and deduplicated results,
  rate limits surface as tool failures.
- Added `browser_read`: headless Chrome rendering and text extraction for public
  pages; clicks, typing and screenshots are not implemented.
- Added `read_pdf`: extracts PDF text through the system `pdftotext`, with read
  root, symlink, control-character and character-count checks. No OCR or image
  understanding.
- Added a stdio MCP client (`--mcp-config`): tool discovery and calls; remote
  tools are write-gated unless the server declares `readOnlyHint: true`.
- Added `spawn_agent`: bounded child processes with isolated journal/workspace
  and budget, inherited write/network permissions and a depth cap of 2.

### 2. Always-on execution, scheduling and skill distribution

- Added `schedule add/list/remove` and `daemon`: intervals, file-mtime triggers,
  persistence, pid locking, isolated child runs and restart catch-up.
- Added `schedule health` with `ok` / `overdue` / `failing` states.
- The daemon can write a `schedule_run_failed` alert and send a webhook when a
  scheduled run fails or times out; escalation and restart deduplication remain
  future work.
- `run/schedule add --routine-skill NAME@VERSION` accepts only the active exact
  version, composes its body into the task and grants no extra permissions.
- Added `skill export/import`: versioned bundles, SHA-256 pinning, manifest
  preflight, a 4 MiB cap and duplicate-version skipping.
- Added Ed25519 bundle signing: `skill keygen`, `skill export --signing-key`,
  `skill import --public-key`. Private key files must be 0600; missing keys,
  wrong keys and tampering fail closed.
- Exposed `skill_list/eval/register/activate/rollback` and
  `delegation_list/register/revoke` tools.

### 3. Cost, maintenance and audit

- `cost` aggregates tokens, cost, tool calls/failures and p50/p95 latency by
  provider/model, task and tool.
- `digest` summarizes recent runs, stop reasons, schedules, `schedule_health`
  and alert counts without calling a model.
- Added `dashboard`: a self-contained static HTML report over journals,
  schedules and alerts, with escaped dynamic text, a restrictive CSP and 0600
  file permissions.
- Audit export supports `after_seq` pagination and `include_records`; the
  default still returns only a receipt so journal content is not dumped
  accidentally.
- Added `retention` (run-journal dry-run/apply with a receipt), `provider-alerts`
  and `notify` (generic webhook plus Slack/Discord payloads).
- Added `memory consolidate` (scope + content-hash deduplication with soft
  forget) and a fingerprint-keyed context-graph cache.

### 4. Safety and execution boundaries

- Tools are filtered by permission; write tools are hidden from the model in
  read-only runs.
- Terminal, file, network, MCP and skill tools have explicit risk levels,
  timeouts and failure paths.
- Tool calls and results are paired in the journal; model-visible inputs can be
  reconstructed from the run journal.
- ADR-0031/0032/0033 record the decisions for execution by default, PDF text
  extraction and skill bundles/routines.

## Published assets

- `1agent-0.96.0-linux-x86_64.tar.gz` — Linux x86_64 package;
- `SHA256SUMS` — archive and Linux provenance checksums;
- `linux-x86_64.json` — Linux build provenance.

Windows archives are not published. GitHub's generated source archives remain
available from the tag.

## Upgrade notes

- 0.96.0 allows local writes/execution by default. Use `--read-only`, or
  `/permissions write off` in the TUI, for read-only runs.
- Skill CLIs honor `ONEAGENT_SKILL_JOURNAL`; the default is
  `$HOME/.1a/skills.jsonl`.
- `--allow-write` is still accepted but conflicts with `--read-only`; it is no
  longer required.
- Existing runtime homes continue to migrate through `1agent upgrade`; this
  release adds no home migration step.

## Verification

- Full gate: fmt, clippy `-D warnings`, tests, evals and claims structure all
  pass.
- 725 tests passed, 0 failed, 5 live tests ignored by default (terminal, PDF and
  other host-dependent checks).
- Claims: 81 checked, 62 verified, 7 partial, 12 without evidence. Unverified
  older product claims remain explicitly marked in the table.
- `1agent --version` reports 0.96.0; Linux reproducible-build provenance ships
  with the package.
- Live evidence: `docs/evidence/web-2026-11-09/`, `mcp-2026-11-09/`,
  `subagent-2026-11-09/`, `browser-2026-11-09/`, `terminal-2026-11-09/`,
  `pdf-2026-11-09/`.

## Known limits

- Not production-stable. A-Gate/S-Gate, external IdP, third-party security audit
  and the Workshop full release gate remain open.
- The browser is read-only; computer-use clicks, typing and screenshots are not
  implemented.
- PDF support is text-only; scan OCR, images, voice, DOCX/PPTX are not
  implemented.
- Semantic retrieval/embeddings and cross-process memory rotation are not
  implemented.
- Skill shadow/canary end-to-end flows and a real demonstration corpus are
  unfinished.
- Delegation is not wired through the full Router path; per-tenant context and
  organization audit are not implemented.
- Watchdog escalation, restart deduplication and a real multi-day task evidence
  run are still missing.
- Remote skill marketplace index, revocation and install UI are not implemented.
- Real external webhook/email/calendar delivery is unverified; the dashboard is
  a static report, not a live service.
- Windows is untested and not published; macOS, Android and iOS are planned.

## License

MIT. See `LICENSE`, `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES.md`.
