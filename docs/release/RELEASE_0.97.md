# 1Agent 0.97.0 — Reliable Loop

[简体中文](RELEASE_0.97.md) · [English](RELEASE_0.97.en.md)

发布日期：2026-11-14  
状态：**R1 release candidate（本地包已生成，公开发布待确认）**  
平台：**Linux x86_64**；Windows 内部构建、不发布；macOS 未支持

> 这是一份“功能与使用方式”的公开入口，不是“生产稳定版”承诺。
> 0.97 是 R1 预发布线，不是 1.0，也不是已经通过第三方审计的 Super-Agent。

## 1. 这是什么

1Agent 是本地优先、可审计、可回放的 Agent 运行时。它把一次任务拆成：

**目的 → 证据 → 授权 → 执行 → 验收 → 审计**

它不只会聊天：接上 provider 和工具后，可以读文件、查网页、写文件、跑终端、开子 Agent、走 MCP；每一步都写进 journal，失败可以回放，写操作有权限门和 kill switch。

0.97 的重点是**长时程可靠闭环**，不是功能数量：

- AG-23 智能闭环：`describe_self`、`request_tools`、`task_need`、证据再规划、策略记忆；
- 流式输出、并行只读工具、增量 journal、checkpoint/request 差分；
- 崩溃恢复、2h 注入 soak、预算/成本门；
- 150+ 条工具与权限边界测试、100 题闭环评测、1h 真实任务门；
- 一轮独立代码级安全评审（0 P0，5 类 P1 已修，残余项公开）。

## 2. 5 分钟快速开始

### 2.1 安装（Linux x86_64）

```bash
tar -xzf 1agent-0.97.0-linux-x86_64.tar.gz
cd 1agent-0.97.0
./scripts/install.sh --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
1agent --version
1agent init
1agent doctor
```

`install.sh` 会先核对包内 `SHA256SUMS`，再原子替换二进制；`init` 只创建 `$HOME/.1a` 运行时目录，不覆盖已有数据。

### 2.2 配一个 provider

DeepSeek / OpenAI-compatible：

```bash
export PI_DEEPSEEK_API_KEY="<your-key>"
1agent run --task "用三句话解释 journal 为什么是可审计的" \
  --config configs/pi-deepseek.json --json
```

本地 Ollama（不联网）：

```bash
ollama serve
ollama pull qwen3:8b
1agent run --task "用一句话介绍你自己" \
  --config configs/ollama-local.json --json
```

也支持 TUI 内 `/model` 面板：掩码输入 key，验证 `/v1/models`，选模型并保存到 `$HOME/.1a/keys/`（0600）。

### 2.3 第一次 TUI

```bash
1agent tui
```

- 输入任务，回车执行；`/help` 看命令；`/status` 看模型与预算；
- `/model` 配置 provider；`/permissions` 切换读写；
- `/clarify` 打开批量追问面板；`/why` 展开最近回答的证据/假设/未知/翻转条件；
- `Ctrl+Q` 两次退出（第一次进入待确认，`Enter`/`Esc` 取消）。

### 2.4 只读运行与审计

```bash
1agent run --task "读 Cargo.toml，报告版本号" \
  --read-only --read-root . --json

1agent audit --journal "$HOME/.1a/journals/<run-id>.jsonl" --json
1agent replay --journal "$HOME/.1a/journals/<run-id>.jsonl" --json
```

`--read-only` 关闭写文件和终端执行；网络仍需 `--allow-network`。`audit` 校验工具调用/观察配对、答案结构和事件序列；`replay` 从 journal 重建运行状态。

### 2.5 卸载

```bash
1agent uninstall --prefix "$HOME/.local"
```

默认只移除二进制；`--purge --yes` 才会删除运行时数据。卸载前请先 `1agent export` 备份。

## 3. 功能矩阵（摘要）

完整表见 [FEATURES-0.97.md](FEATURES-0.97.md)。状态含义：

- ✅ 可用：有测试/实机证据，按文档使用
- ◐ 部分可用：主路径可用，边界明确
- 🧪 实验：接口/行为可能变化
- ✖ 未实现：不要按“已有”理解

| 类别 | 能力 | 状态 | 入口 |
|---|---|---|---|
| 入口 | TUI / headless / 本地 HTTP surface | ✅ | `tui`、`run`、`serve` |
| 任务 | 任务图、Ticket、Mandate、审批、SideEffect、Receipt | ◐ | `task-graph`、`approval`、`receipt` |
| 审计 | journal、audit、replay、metrics、org audit | ✅/◐ | `audit`、`replay`、`metrics`、`org-audit` |
| 智能闭环 | `describe_self`、`request_tools`、证据再规划、策略草案 | ✅ | 运行时工具 + `/why` |
| 文件 | 读文本/PDF、列目录、写文件、编辑 | ✅ | `read_file`、`read_pdf`、`search_files`、`write_file`、`edit_file` |
| 终端 | 真实 PTY 创建/写入/执行/销毁 | ◐ | `terminal_*` 工具；需写权限 |
| 网络 | `fetch_url`、`web_search`、`browser_read` | ✅/◐ | `--allow-network` |
| MCP | stdio + streamable HTTP、token file/exchange | ◐ | `--mcp-config`、`mcp-token-*` |
| 子 Agent | 有界子进程、独立 journal/工作区、深度 2 | ✅ | `spawn_agent` |
| 记忆 | 四层记忆、scope/taint/TTL、软/硬遗忘 | ✅ | `memory`、`user-model` |
| 技能 | SKILL.md、扫描、评测、激活、回滚、导出 | ◐ | `skill` |
| 进化 | Reflection/Lesson、Proposal、Ledger、人工审批 | ◐ | `experience`、`approval` |
| 调度 | interval/trigger、daemon、health、watchdog | ◐ | `schedule`、`daemon`、`watchdog` |
| Provider | OpenAI-compatible、Ollama、路由、成本账本 | ✅ | `providers`、`route-plan`、`cost` |
| 成本 | 预算、预估、checkpoint、熔断、`--max-cost-micros` | ✅ | `benchmark-run`、`budget-check` |
| 安全 | 权限、Authority、审批、kill switch、read-only | ✅ | `--read-only`、`--authority`、`~/.1a/KILL` |
| 平台 | Linux 包、SHA256、provenance | ✅ | `package.sh`、`provenance` |
| 未实现 | computer-use 点击/输入/截图、语音/图片理解、多租户、移动端、外部 IdP | ✖ | — |

## 4. 八个常用场景

### 4.1 调研并给出出处

```bash
1agent run \
  --task "调研 SQLite WAL 与 rollback journal 的读写并发差异，列出实际来源" \
  --allow-network --config configs/pi-deepseek.json --json
```

适合：技术选型、竞品/文档调研。注意：来源抓取受网络和页面结构影响；`browser_read` 不要用于不可信页面。

### 4.2 做一个决策

```bash
1agent run \
  --task "团队在‘继续补功能’和‘补真实证据’之间只能选一个，建议哪个？给出排序和翻转条件" \
  --json
```

输出应包含结论、理由、假设、未知和翻转条件；`/why` 可展开完整结构。

### 4.3 读本地文件/目录

```bash
1agent run \
  --task "找出 src 下所有包含 TODO 的文件，按模块归类" \
  --read-only --read-root . --json
```

`search_files` 空 query 只列路径，不读内容；适合先发现再读取。

### 4.4 写文件并验证

```bash
1agent run \
  --task "在 /tmp 下创建三行文本文件，报告路径和 SHA-256" \
  --allow-write --json
```

写操作会进 journal 和 side-effect 记录；高风险动作仍受 Authority/审批约束。

### 4.5 终端执行

```bash
1agent run \
  --task "运行 git status --short，只报告是否有未提交改动" \
  --allow-write --json
```

终端工具走真实 PTY；依赖 Skynet Workshop 的执行底座时，缺少底座会返回可行动错误。

### 4.6 接 MCP

```bash
1agent run \
  --task "用 filesystem MCP 列出当前目录" \
  --mcp-config configs/mcp.example.json \
  --allow-network --allow-write --json
```

0.97 的所有 MCP 工具都按写工具处理，且需要网络权限；stdio server 只继承最小环境变量，不继承 provider key。

### 4.7 派一个子 Agent

```bash
1agent run \
  --task "把‘整理 0.97 发布说明’拆给一个子 Agent，父 Agent 只做验收" \
  --allow-write --json
```

子 Agent 有独立 journal/工作区/预算，深度上限 2；`spawn_agent` 自身按写工具分类，只读/KILL/Authority 写门会先拦。

### 4.8 定时任务与守护

```bash
1agent schedule add --name daily \
  --every-seconds 86400 --task "总结今天的 workspace 变化"
1agent schedule health
1agent daemon --loop
```

支持 interval、文件 trigger、重启补跑、失败告警 webhook；自动 Watchdog 升级策略仍不完整，不要把它当系统级守护进程。

## 5. TUI / CLI 速查

| 操作 | 命令 |
|---|---|
| 看帮助 | `/help` |
| 配模型 | `/model` |
| 看/切权限 | `/permissions`、`/permissions write on` |
| 批量追问 | `/clarify` |
| 展开证据 | `/why` |
| 看运行状态 | `/status` |
| 切认知模式 | `/mode` |
| 看详情 | `/details` |
| 清屏（需确认） | `/clear` |
| 退出 | `Ctrl+Q` 两次 |

CLI 常用：

```bash
1agent run --help
1agent benchmark-run --help
1agent eval --suite evals/core.json --json
1agent metrics --journal-dir "$HOME/.1a/journals" --json
1agent answer-quality --input samples.jsonl --json
```

## 6. 证据与指标（本地）

| 项 | 结果 | 说明 |
|---|---|---|
| 1h 真实任务门 | M1 **97.08%**（233/240），suite 76.66%，M5 **100%**（239/239） | DeepSeek `deepseek-flash`，44m16s，约 ¥11.63；一次 provider error 丢 case 记录，CLI 显示失败 |
| 8h 本地运行时 soak | 56 轮 / 448 任务，**0 崩溃**，replay 234/234 | Ollama `qwen3-8b-16k`，8h26m51s；本地模型质量 21.2%，不作为 M1 |
| 100 题 AG-23 | **96/100** | intro 20/20、追问 20/20、高风险 20/20、简单聊天 19/20、最新/本地 17/20 |
| 2h 崩溃注入 soak | 28,552 cycles / 7,200.20s / exit 0 | 副作用不重复、无 checkpoint 的 journal 失败关闭 |
| 独立代码级安全评审 | **0 P0**；5 类 P1 已修；4 项 P1 残余 | 不是第三方审计/认证 |
| M19–M23 | M19 85.00%，M20 5.13%（2/39），M21 2.50%，M22 0.00%，M23 null | 标签是 AI/批量写入，**不是人工盲评** |

证据入口：`docs/evidence/`。完整状态：`docs/status-v0.97.md`；安全：`docs/audit-v0.97.md`；指标复核：`docs/status-v0.97.md`。

## 7. 安全边界（发布必读）

- **写/执行默认开启**（本地 CLI/TUI）；`--read-only` 关闭写和终端；网络需 `--allow-network`。
- **Authority/Policy/Approval** 可进一步限制工具、风险、强度和范围；`RequireApproval` 在 0.97 是失败关闭，不会静默执行。
- **kill switch**：`$HOME/.1a/KILL` 存在时写工具失败关闭；它不杀进程、不重启、不清理锁。
- **MCP**：所有 MCP 工具按写工具处理，需要写权限和网络权限；不信任 server 的 `readOnlyHint`。
- **browser_read**：只渲染读取，不点击/输入；Chrome 会自行解析 DNS 和跟随重定向，**不要用于不可信页面**（已知 P1）。
- **run journal 没有逐事件哈希链**：本机写者可以改写 journal；需要更强证据时用 org audit/导出。已知 P1。
- **第三方审计/认证、人工盲评、macOS P-Gate 未完成**；不要把本地代码级评审写成“通过安全审计”。

## 8. 升级与回滚

1. 备份 `$HOME/.1a` 和 journal 目录。
2. 0.97 可以读取 0.96.1 journal；**0.96.1 不能读取 0.97 的 `checkpoint_delta`/`model_request_delta` journal**。
3. 回滚前先 `1agent export`；回滚步骤见 `docs/rollback-v0.97.md`。
4. 升级/回滚后跑 `1agent doctor` 和 `1agent audit` 抽样。

## 9. 已知问题与反馈

已知边界：computer-use、语音/图片理解、多租户、组织级审计、移动端未实现；Windows 不发布；macOS 未支持。

反馈：

- [Skynet-Club](https://skynet.club) — 产品讨论、试用反馈、共建；
- GitHub Issues — 附 `1agent doctor`、`1agent --version`、最小复现和 journal 路径（不要贴 key）。

## 10. 发布资产

- `1agent-0.97.0-linux-x86_64.tar.gz`
- `SHA256SUMS`
- `linux-x86_64.json`（provenance）

本地包二进制 SHA-256：`36c6e44faa15e4011261f20b735738cd7fb05a9b9818ddd0340e0fad482ee3fb`。公开发布前仍需重建干净历史、确认资产并完成人工盲评/第三方认证。
