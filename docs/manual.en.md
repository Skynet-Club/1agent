# Manual

> Version: **0.96.1 R1 release candidate**  
> Platform: **Linux x86_64**  
> Windows is untested and not published; macOS, Android and iOS are planned.

[简体中文](manual.md) · **English**

## 1. Install

### From the release archive

```bash
tar -xzf 1agent-0.96.1-linux-x86_64.tar.gz
cd 1agent-0.96.1
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
| `Ctrl+O` | Open/close the run details and process trace |
| `Ctrl+Q` | Request exit; `Enter` confirms, `Esc` cancels |
| `Ctrl+C` | Copy selected text |

The session input box uses a dark-green panel whose background, border and panel text are dimmed by 50%. Input text stays white. While the width allows, token metrics sit on the right side of the input box's top border:

```text
╭─ • 模型思考 Thinking · 等待响应 · 第 1 步 ───── • ↑ 1.2k / ↓ 340 · 缓存 40% · 上下文 ───── 13% · 12s ────╮
│                                                                                                  │
│ › Type a task…                                                                                   │
│                                                                                                  │
╰─ ● 运行中 · deepseek-flash · 直接回答 · Ctrl+T 模式 ────────────────────────────────────────────╯
```

On a narrow terminal the state heading stays on the top border and the metrics fold onto their own row inside the same fence instead of being truncated:

```text
╭─ • 模型思考 Thinking · 等待响应 · 第 1 步 ─────────╮
│ ↑ 1.2k / ↓ 340 · 缓存 40% · 上下文 ─────── 13% · 12s │
│                                                     │
│ › Type a task…                                      │
│                                                     │
╰─ ● 运行中 · deepseek-flash · 直接回答 · Ctrl+T 模式 ───────────────╯
```

- The left side of the top border comes from the run journal and shows the current stage, step and bilingual state.
- While the width allows, the right side of the top border embeds token arrows, cache hit rate, context meter and elapsed time; otherwise they fold to the row below.
- The bottom border shows run status, model and session mode, without repeating the context percentage (the top border already has the meter); the `T` in `Ctrl+T` is bright green and bold.
- The whole TUI backdrop is `#06100A` (80% of the input panel's green brightness), slightly darker than the input panel's `#08140D`; the welcome screen, transcript and command panel share it.
- Input-panel background is `#08140D`, the border is `#3F644B`, and panel text is `#487748`; input text is white, all unbolded.
- The context meter uses `─` throughout, matching the fence border: panel green `#487748` for used cells, black for the remaining cells. It shows 20 cells on wide terminals and half that (10) on narrow ones, without changing the displayed percentage. It uses a token budget (1M by default, auto-compacting at 786,432 tokens).
- Master-Agent next-command footers appear on the right side of the top border, or with the metrics when folded.

The same journal events are appended incrementally to the main transcript as a visible work log, so tool calls and results are not hidden in the status strip:

```text
 • 任务理解 Task Understanding · 解析目标与约束
 • 规划 Planning · 任务计划已生成
 • 模型思考 Thinking · 第 1 步
   ⎿ 请求 8 条消息 · 上下文 12400 tokens · 工具 12 个
   ⎿ 返回工具调用 · read_file
 • 读取 Reading · read_file · /home/skynet/1-agent/README.md
   ⎿ read README.md (999 bytes)
```

Tool calls start with `•`, which is always bright green; action names such as `模型思考 Thinking`, `执行命令 Bash` and `整理答案 Answering` are bright green too, and the name and detail are separated by ` · `. Tool names, arguments and `⎿` results use deep green `#487748`, none of them bolded. Each thinking step also prints `⎿` summaries for the request (messages/context/tools) and the response (final answer or tool calls); `Ctrl+O` opens the run details and process trace. Links in the work log — local file paths (`/path/README.md`, `src/main.rs`, …) and http(s) URLs — use bright green. Context counts appear only in the thinking request summary and the input fence, not in tool rows. Bash results include up to three lines / 240 characters of output preview; paths, search queries and subagent tasks are shown with their tool names.

User messages render on the input panel's `#08140D` background with white text, matching the composer.

Short replies in the same session are resolved against the previous request and answer. If the previous turn asked “who is Feli”, a reply of `felix` continues that thread instead of starting a new search. Prompt-author names are checked against the 48-entry local prompt library (Felix Rieseberg, Karpathy, …) before the web.

Right-click pastes from the clipboard only when the pointer is inside the composer; right-clicking the transcript, work log or any other region does nothing. `Ctrl+V` is not restricted by pointer position.

All popup surfaces (`/details`, `/help`, the `/` command drawer and the `Ctrl+T` mode menu) share one green family: surface `#0A1F12`, border `#3F644B`, bright-green titles and commands, and a `#143D24` selected row. Blue labels and gray panel surfaces are gone.

The final answer is wrapped in its own fence titled `结论 Conclusion`, with one blank line of padding above and below the body. The fence border is dimmed deep green `#1F4E28`, the `结论 Conclusion` title is bright green and bold, and the body is white; the following claims/evidence/assumptions/constraints/unknowns/conflicts/flip-condition sections use deep green `#487748` headings, labels, bullets and body text; only the answer inside the fence stays white.

The answer body supports Markdown hierarchy: `##` subheadings, ordered and unordered lists, `- [ ]` task boxes, `**bold**` key terms, `---` dividers, tables and links; tables are padded to aligned columns before rendering, and URLs or local paths use the link color. The model is asked to lead with one result sentence and then use short paragraphs, bullets or a comparison table for multi-dimensional answers instead of one long wall of text.

```text
╭─ 结论 Conclusion ─────────────────────────────────────────────╮

   不用启动，GridCraft 0.16.0 此刻已经在运行。

╰───────────────────────────────────────────────────────────────╯
```

Drag with the left mouse button in the transcript or composer to select text; `Ctrl+C` copies it.

Full state vocabulary:

- Cognition: 任务理解 Task Understanding, 规划 Planning, 选择模型 Model Routing, 模型思考 Thinking, 压缩上下文 Compacting, 整理答案 Answering, 验收 Verifying, 重试 Retrying, 阻塞 Blocked
- Perception: 读取 Reading, 写入 Writing, 创建 Creating, 修改 Updating, 搜索 Searching, 抓取 Fetching, 查询 Querying
- Execution: 执行命令 Bash, 子 Agent Subagent, 技能 Skill, 委托 Delegation, 调用工具 Tool Call, 工具结果 Tool Result
- Master: 任务理解 Mission Understanding, 任务规划 Mission Planning, 等待批准 Waiting Approval, 任务分配 Task Assignment, 任务启动 Task Starting, Worker 运行 Worker Running, 收集结果 Collecting Results, 任务验收 Mission Verification, 汇报 Reporting

A bare `1agent tui` starts a new session. The exit banner prints the current session title and the explicit continue command:

```bash
1agent tui --session <id>
```

### Command drawer

Input beginning with `/` opens the command drawer:

- empty query groups commands under `Recent / Session / Thinking / Evidence / Model & Permissions / Master`;
- the three most recently accepted commands are shown first;
- a keyword such as `/验收`, `/证据` or `/老板` searches both commands and the 48 built-in prompt supplements;
- the empty drawer starts with 3 featured prompts so commands stay visible; the other 45 are searchable by keyword;
- with an empty composer, type `//` to open the complete source library: the 56 verbatim Feishu rows are grouped into 32 author/topic entries and labeled by category. Insertion is the complete body only: the Chinese translation in Chinese mode or the English original in English mode; nothing is truncated or rewritten, and author, topic, category and source link stay in the drawer row rather than being written into the composer. On wide terminals a side pane previews the selected group's complete body, author, category and character count before you press `Enter`. Inserting the same body twice is skipped. `//验收` filters inside the source library;
- the `/` drawer still keeps the 48 curated cards for a quick keyword search; use `//` when you want the expert's exact words;
- after typing a task, add a space and `//` (for example `做跨境电商 //验收`) to open the prompt drawer; the selected template is prepended or appended per its entry, and `Esc` restores the task;
- use `↑` / `↓` to select, `Enter` to insert and `Esc` to close;
- selecting a prompt fills the composer and leaves `{{variables}}` for editing; each row shows its value level, category, purpose and anti-use;
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

The interactive TUI has no default limit on steps, tool calls, provider attempts or run duration (`0` means unlimited), so long tasks that drive external CLIs such as `lark-cli` can finish. Headless `run`, the daemon and subagents keep finite budgets. Tighten a session with `/config set steps 20` or `/config set timeout 300`; `/config reset` restores the defaults. Each model HTTP call still has a 300-second transport ceiling.

The prompt drawer supports `/config set language zh|en`. Chinese mode shows the curated Chinese cards. English mode lists and inserts only the 14 cards whose instruction text exists verbatim in the Feishu source table; cards without an English source stay hidden instead of silently falling back to Chinese.

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
| `--max-context-tokens` | 1000000 | Context token budget; auto-compact at 786,432 |
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
