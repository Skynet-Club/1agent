# 1Agent 0.97.0 — Reliable Loop

[简体中文](RELEASE_0.97.md) · [English](RELEASE_0.97.en.md)

Released: 2026-11-14  
Status: **R1 release candidate (local package built; public release pending)**  
Platform: **Linux x86_64**; Windows is an internal build and is not published;
macOS is not supported.

> This is a public entry point for features and usage, not a production-stability
> promise. 0.97 is an R1 pre-release, not 1.0, and not a third-party-audited
> Super-Agent.

## 1. What it is

1Agent is a local-first, auditable, replayable agent runtime. It turns a task
into:

**intent → evidence → authorization → execution → acceptance → audit**

It can read files, search the web, write files, run a PTY, spawn bounded child
agents and use MCP servers. Every model-visible input, tool call and result is
journaled; completed runs can be audited and replayed.

0.97 focuses on the reliable loop: AG-23 (`describe_self`, `request_tools`,
`task_need`, evidence re-planning, strategy drafts), streaming, parallel
read-only tools, incremental journals, checkpoints, crash recovery, cost
governance and a code-level security review.

## 2. Quick start (Linux x86_64)

```bash
tar -xzf 1agent-0.97.0-linux-x86_64.tar.gz
cd 1agent-0.97.0
./scripts/install.sh --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
1agent --version
1agent init
1agent doctor
```

Install verifies the packaged `SHA256SUMS` before replacing the binary. `init`
only creates `$HOME/.1a`; it does not overwrite existing data.

Configure a provider:

```bash
# OpenAI-compatible / DeepSeek
export PI_DEEPSEEK_API_KEY="<your-key>"
1agent run --task "Explain why the journal is auditable in three sentences" \
  --config configs/pi-deepseek.json --json

# Local Ollama
ollama serve
ollama pull qwen3:8b
1agent run --task "Introduce yourself in one sentence" \
  --config configs/ollama-local.json --json
```

First TUI:

```bash
1agent tui
```

`/help` shows commands, `/model` configures a provider, `/status` shows the
model and budgets, `/why` expands the last answer's evidence, `/clarify` opens
the batch question panel, and `Ctrl+Q` twice exits.

Read-only run and audit:

```bash
1agent run --task "Read Cargo.toml and report the version" \
  --read-only --read-root . --json

1agent audit --journal "$HOME/.1a/journals/<run-id>.jsonl" --json
1agent replay --journal "$HOME/.1a/journals/<run-id>.jsonl" --json
```

## 3. Feature summary

The full matrix is in [FEATURES-0.97.md](FEATURES-0.97.md) (Chinese). Status:
✅ available, ◐ partial, 🧪 experimental, ✖ not implemented.

| Area | What is available | Status |
|---|---|---|
| Entry points | TUI, headless `run`, local `serve` | ✅/◐ |
| Tasks | task graph, Ticket, Mandate, approval, SideEffect, Receipt | ◐ |
| Audit | journal, audit, replay, metrics, org audit | ✅/◐ |
| AG-23 loop | `describe_self`, `request_tools`, evidence re-plan, strategy drafts | ✅ |
| Files | read text/PDF, list files, write, edit | ✅ |
| Terminal | real PTY create/write/exec/destroy | ◐ |
| Network | `fetch_url`, `web_search`, `browser_read` | ✅/◐ |
| MCP | stdio + streamable HTTP, token file/exchange | ◐ |
| Subagents | bounded child, depth 2, isolated journal/workspace | ✅ |
| Memory | four layers, scope/taint/TTL, soft/hard forget | ✅ |
| Skills | SKILL.md, scan, eval, activate, rollback, export | ◐ |
| Scheduling | interval/trigger, daemon, health, watchdog | ◐ |
| Providers | OpenAI-compatible, Ollama, routing, cost ledger | ✅ |
| Security | permissions, Authority, approval, kill switch, read-only | ✅ |
| Release | Linux package, SHA256, provenance | ✅ |
| Not implemented | computer-use, voice/image, multi-tenant, mobile, external IdP | ✖ |

## 4. Eight recipes

1. **Research with sources**

   ```bash
   1agent run --task "Compare SQLite WAL and rollback journal concurrency; list the sources you actually found" \
     --allow-network --config configs/pi-deepseek.json --json
   ```

2. **Make a decision**

   ```bash
   1agent run --task "Choose between adding features and adding evidence; give an order and flip conditions" --json
   ```

3. **Read local files**

   ```bash
   1agent run --task "Find TODO comments under src and group them by module" \
     --read-only --read-root . --json
   ```

4. **Write and verify a file**

   ```bash
   1agent run --task "Create a three-line file under /tmp and report its path and SHA-256" \
     --allow-write --json
   ```

5. **Run a terminal command**

   ```bash
   1agent run --task "Run git status --short and report only whether there are uncommitted changes" \
     --allow-write --json
   ```

6. **Use MCP**

   ```bash
   1agent run --task "Use the filesystem MCP to list the current directory" \
     --mcp-config configs/mcp.example.json --allow-network --allow-write --json
   ```

7. **Spawn a subagent**

   ```bash
   1agent run --task "Split the release-note cleanup into a child agent; the parent only accepts the result" \
     --allow-write --json
   ```

8. **Schedule and daemon**

   ```bash
   1agent schedule add --name daily --every-seconds 86400 --task "Summarize today's workspace changes"
   1agent schedule health
   1agent daemon --loop
   ```

## 5. Evidence (local)

| Item | Result | Note |
|---|---|---|
| 1h real-task gate | M1 **97.08%** (233/240), suite 76.66%, M5 **100%** | DeepSeek `deepseek-flash`, 44m16s, ~¥11.63; one provider error lost a case record |
| 8h local runtime soak | 56 runs / 448 tasks, **0 crashes**, replay 234/234 | Ollama `qwen3-8b-16k`, 8h26m51s; local model quality 21.2%, not M1 |
| 100-question AG-23 | **96/100** | intro 20/20, follow-up 20/20, high-risk 20/20, chat 19/20, current/local 17/20 |
| 2h crash-injection soak | 28,552 cycles / 7,200.20s / exit 0 | no duplicate side effects; fail-closed without a checkpoint |
| Independent code-level review | **0 P0**; five P1 classes fixed; four P1 residual | not a third-party audit |
| M19–M23 | M19 85.00%, M20 5.13% (2/39), M21 2.50%, M22 0.00%, M23 null | labels are AI/bulk-written, **not a human blind review** |

See `docs/evidence/`, `docs/status-v0.97.md` and `docs/audit-v0.97.md`.

## 6. Security boundaries

- Local CLI/TUI enables writes and execution by default; `--read-only` disables
  both. Network requires `--allow-network`.
- Authority/Policy/Approval can restrict tools, risk, strength and scope.
  `RequireApproval` is fail-closed in 0.97; it never executes silently.
- `$HOME/.1a/KILL` blocks write tools; it does not kill processes, restart a
  daemon or clear locks.
- Every MCP tool is write-gated and requires network permission; server
  `readOnlyHint` is not trusted. Stdio servers do not inherit provider keys.
- `browser_read` renders and reads but does not click/type; Chrome resolves DNS
  and follows redirects independently, so **do not use it on untrusted pages**
  (known P1).
- Run journals do not have a per-event hash chain; a local writer can rewrite
  them. Use org audit/export when stronger evidence is required (known P1).
- Third-party audit/certification, a human blind review and macOS P-Gate are
  still open. Do not describe the code-level review as a passed security audit.

## 7. Upgrade and rollback

1. Back up `$HOME/.1a` and journal directories.
2. 0.97 reads 0.96.1 journals; **0.96.1 cannot read 0.97 journals with
   `checkpoint_delta`/`model_request_delta`**.
3. Export before rollback; see `docs/rollback-v0.97.md`.
4. Run `1agent doctor` and sample `1agent audit` after upgrading or rolling
   back.

## 8. Release assets

- `1agent-0.97.0-linux-x86_64.tar.gz`
- `SHA256SUMS`
- `linux-x86_64.json` (provenance)

Binary SHA-256:
`36c6e44faa15e4011261f20b735738cd7fb05a9b9818ddd0340e0fad482ee3fb`.
A public release still needs rebuilt clean history, asset confirmation, a human
blind review and third-party certification.
