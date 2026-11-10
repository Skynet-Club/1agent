# Manual

> Version: **0.96.0 R1 release candidate**  
> Platform: **Linux x86_64**  
> Windows is untested and not published; macOS, Android and iOS are planned.

[简体中文](manual.md) · **English**

## 1. Install

### From the release archive

```bash
tar -xzf 1agent-0.96.0-linux-x86_64.tar.gz
cd 1agent-0.96.0
./scripts/install.sh --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
1agent --version
```

The installer only copies the binary. It does not initialize or delete data. Do not run 1Agent as root; the default runtime home belongs to the current user.

### From source (developers)

```bash
git clone https://github.com/Skynet-Club/1agent.git
cd 1agent
cargo build --release --offline
target/release/first-principles-agent --version
```

## 2. Initialize and check

```bash
1agent init
1agent doctor
1agent doctor --json
```

Default runtime home:

- `$ONEAGENT_HOME` if set;
- otherwise `$HOME/.1a`.

`doctor` checks the home layout, key directory, journals and actionable problems. Do not edit `home.json` by hand; use `1agent upgrade` for migration.

## 3. First run

### Offline (recommended)

```bash
1agent tui
```

The default deterministic provider needs no network or API key. TUI sessions are written to `--journal-dir` (default `.journal`); use an explicit directory when needed:

```bash
1agent tui --journal-dir "$HOME/.1a/journals"
```

Headless:

```bash
1agent run --task "Explain the purpose and boundaries of this project"
```

### Connect a real model

Keep credentials in environment variables, not configuration files:

```bash
export DEEPSEEK_API_KEY="..."
1agent tui --config configs/example.json --allow-network
```

Or with CLI flags:

```bash
export AGENT_API_KEY="..."
1agent run \
  --task "Compare option A and option B" \
  --provider openai-compatible \
  --base-url https://api.deepseek.com/v1 \
  --model deepseek-chat \
  --allow-network
```

Check provider configuration:

```bash
1agent providers --config configs/example.json --check
1agent providers --config configs/example.json --probe --allow-network
```

`--probe` makes network calls; only use it with endpoints you trust. The example configuration references credentials through `api_key_env`.

## 4. TUI basics

| Key | Action |
|---|---|
| `Enter` | Submit a task or execute a command |
| `Tab` | Complete a `/` command |
| `Up / Down` | Move in a menu, or scroll |
| `Esc` | Close a panel/drawer and return to the conversation |
| `Ctrl+T` | Open the conversation mode menu |
| `Ctrl+K` | Open the `/` command drawer |
| `Ctrl+Q` | Request exit; `Enter` confirms, `Esc` cancels |
| `Ctrl+C` | Copy selected text |

While running, the line above the composer shows:

```text
Working... 12s · ↑1.2k ↓340 · cache 40% · Ctrl+C 取消
```

Token and cache values come from provider usage. If the provider does not report cache tokens, the line shows `cache —`.

A bare `1agent tui` starts a new session. The exit banner prints the current session title and the explicit continue command:

```bash
1agent tui --session <id>
```

### Command drawer

Input beginning with `/` opens the command drawer:

- empty query groups commands under `Recent / Session / Thinking / Evidence / Model & Permissions / Master`;
- the three most recently accepted commands are shown first;
- unavailable commands are muted with a reason;
- `Tab` completes a command such as `/frame `;
- `Enter` on an incomplete command only completes it; a second Enter executes;
- `Esc` closes the drawer but keeps the input.

### Common commands

| Command | Purpose |
|---|---|
| `/help` | Commands and current scope |
| `/status` | Run status, host and budget |
| `/frame` | Facts, assumptions, constraints, unknowns and acceptance criteria |
| `/challenge` | Red-team the plan, conclusion, assumptions or evidence |
| `/evidence` | claim / evidence / unknown / conflict |
| `/audit` | Audit the current run or Master journal |
| `/report` | Task or production-line report |
| `/mode` | Switch conversation mode |
| `/model` | View or select the next run's model route |
| `/permissions` | View or adjust the next run's permissions |
| `/session` | Session lifecycle |
| `/workspace` | Working directory, files, diff and worktree |
| `/context` | Compact context or generate a summary |
| `/preset` | Reusable task templates |
| `/tools` | ToolRegistry, schemas, authority and risk |
| `/export` | Export session, report or evidence |
| `/copy` | Copy the latest answer |
| `/cost` | Token usage and cost |
| `/config` | TUI configuration |
| `/reframe` | Decide whether the request is a goal or means; ask when needed |

Destructive commands (`/clear`, non-dry-run `/dispatch`, `/control cancel`, `/control takeover`) require a second Enter. Other input cancels the confirmation.

> Master/Workshop commands only appear in a Workshop Master binding; standalone mode hides them.

## 5. Headless runs

```bash
# Offline deterministic provider
1agent run --task "Give a conclusion, evidence and flip conditions"

# Structured JSON
1agent run --task "..." --json

# Authorize a read-only root
1agent run --read-root "$HOME/project" --task "Read and summarize the project structure"

# Configuration + network
1agent run --config configs/example.json --allow-network --task "..."
```

Common limits:

| Option | Default | Purpose |
|---|---|---|
| `--max-steps` | 6 | Maximum reasoning/tool steps |
| `--max-tool-calls` | 8 | Maximum tool calls |
| `--timeout-seconds` | 120 | Per-run timeout |
| `--max-context-chars` | 100000 | Context character budget |
| `--read-root` | current directory | Read-only root; repeatable |
| `--allow-network` | off | Allow provider network access |

## 6. Sessions and workspace

```bash
1agent tui --session <session-id>
1agent tui --journal-dir .journal
```

`/workspace` can:

- show a workspace overview, files and diff;
- switch the authorized root;
- mention paths;
- initialize `AGENTS.md`;
- manage git worktrees.

Paths must stay inside authorized roots; escapes are rejected.

## 7. Export, restore and migration

```bash
# Export without key material
1agent export --output "$HOME/1agent-export"

# Restore
1agent restore --bundle "$HOME/1agent-export"

# Check/upgrade the home
1agent upgrade --check
1agent upgrade
1agent upgrade --rollback
```

Exports include a SHA-256 manifest; restore verifies it. Key backup is a separate local mechanism:

```bash
1agent key escrow --output "$HOME/1agent-keys.escrow.json" \
  --recovery-key "$HOME/1agent-recovery.key"
1agent key restore --escrow "$HOME/1agent-keys.escrow.json" \
  --recovery-key "$HOME/1agent-recovery.key"
```

Keep the escrow and recovery key apart. This is not HSM/quorum custody.

## 8. Uninstall

```bash
# Keep data
1agent uninstall

# Delete data (dangerous, requires confirmation)
1agent uninstall --purge --yes
```

The binary removal does not delete `$HOME/.1a` unless `--purge` is used.

## 9. Local service and integration

### stdio host protocol

```bash
1agent serve
```

See repository `docs/io-contracts.md` for protocol details.

### Surface HTTP (loopback)

```bash
export SURFACE_TOKEN="$(openssl rand -hex 32)"
1agent surface serve \
  --addr 127.0.0.1:8788 \
  --store "$HOME/.1a/task-graph.jsonl" \
  --token "$SURFACE_TOKEN"
```

- binds to loopback by default;
- requires a Bearer token;
- not a public SaaS, not multi-tenant;
- see repository `docs/` and `surface` help for the supported subset.

## 10. Troubleshooting

| Symptom | Action |
|---|---|
| `1agent: command not found` | Ensure `$HOME/.local/bin` is in `PATH` |
| Provider credentials not ready | `1agent providers --config configs/example.json --check` |
| Network request denied | Add `--allow-network` and verify the endpoint |
| File read denied | Use `--read-root` or switch the workspace in `/workspace` |
| TUI rendering issues | Use a real terminal; Linux x86_64 is the validated platform |
| doctor reports journal/home issues | Stop writers, then run `1agent doctor`; do not edit hash chains |
| Lost recovery key | The escrow cannot be decrypted without it; this is by design |

## 11. Current limits

- Browser support is read-only (`browser_read`); computer-use actions are not implemented;
- `fetch_url`, `web_search`, stdio MCP and bounded subagents are MVP features;
- daemon/Routine supports intervals, file triggers, restart catch-up, skill binding and `schedule health`; failed/timed-out runs can write and webhook alerts, but escalation and cross-restart deduplication are not implemented;
- voice/image understanding is not implemented; `read_pdf` extracts PDF text only;
- Workshop execution is R&D/integration;
- Windows is untested and not published; macOS/Android/iOS are planned;
- not production-stable; interfaces and data formats may change.

---

[Back to README](../README.en.md) · [Features](features.en.md) · [Ecosystem](ecosystem.en.md) · [Status](status.en.md)
