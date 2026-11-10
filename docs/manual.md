
# 操作手册

[简体中文](manual.md) · [English](manual.en.md)

> 适用版本：**0.96.0 R1 release candidate**  
> 适用平台：**Linux x86_64**  
> Windows 版本尚未测试，暂不发布、不提供下载；macOS、Android、iOS 后续发布。

## 1. 安装

### 从发布包安装

```bash
tar -xzf 1agent-0.96.0-linux-x86_64.tar.gz
cd 1agent-0.96.0
./scripts/install.sh --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"
1agent --version
```

安装脚本只复制二进制，不会自动初始化或删除数据。  
不要把 `1agent` 当 root 运行；默认使用当前用户的 runtime home。

### 从源码构建（开发者）

```bash
git clone <repository>
cd 1-agent
cargo build --release --offline
target/release/first-principles-agent --version
```

## 2. 初始化与检查

```bash
1agent init
1agent doctor
1agent doctor --json
```

默认 runtime home：

- `$ONEAGENT_HOME`，如果设置了；
- 否则 `$HOME/.1a`。

`doctor` 会检查 home 布局、密钥目录、journal 和可行动问题。  
初始化后的 home 结构大致为：

```text
~/.1a/
  home.json
  keys/
  journals/
  ...
```

不要手动改 `home.json`；升级和迁移用 `1agent upgrade`。

## 3. 第一次运行

### 离线体验（推荐）

```bash
1agent tui
```

默认使用确定性 provider，不需要网络和 API key。  
适合先体验 TUI、命令、journal 和任务流程。

TUI 默认把会话写入 `--journal-dir`（默认 `.journal`）；需要固定位置时显式指定：

```bash
1agent tui --journal-dir "$HOME/.1a/journals"
```

单次 headless：

```bash
1agent run --task "解释这个项目的目标和边界"
```

### 接入真实模型

密钥只放环境变量，不写入配置文件：

```bash
export DEEPSEEK_API_KEY="..."
1agent tui --config configs/example.json --allow-network
```

或使用命令行参数：

```bash
export AGENT_API_KEY="..."
1agent run \
  --task "比较方案 A 和方案 B" \
  --provider openai-compatible \
  --base-url https://api.deepseek.com/v1 \
  --model deepseek-chat \
  --allow-network
```

通知：

```bash
1agent notify --webhook https://example.com/hook --message "任务完成"
```

只接受 HTTPS 或 loopback HTTP；当前是通用 webhook，邮件/IM/日历连接器未实现。

Provider 告警历史：

```bash
1agent provider-alerts --alert-log "$HOME/.1a/provider-alerts.jsonl"
1agent provider-alerts --alert-log "$HOME/.1a/provider-alerts.jsonl" --json
```

按 kind/provider/model 汇总告警日志；运行时通过 `--alert-log` 写入，`--alert-webhook` 负责外发。

记忆整合（默认 dry-run）：

```bash
1agent memory consolidate
1agent memory consolidate --apply --json
```

按 scope + content SHA-256 找重复的活跃记忆，保留 `valid_from_ms` 最新的记录，其余软遗忘；hard/soft 已遗忘记录不参与。

保留期（默认 dry-run）：

```bash
1agent retention --journal-dir "$HOME/.1a/journals" --older-than-days 30
1agent retention --journal-dir "$HOME/.1a/journals" --older-than-days 30 --apply
```

只处理顶层 `.jsonl` run journal；`--apply` 会删除过期文件并追加 `retention-receipt.jsonl`。memory/audit 存储的保留期不在这个命令内。

每日/手动摘要：

```bash
1agent digest --journal-dir "$HOME/.1a/journals" \
  --alert-log "$HOME/.1a/schedule-alerts.jsonl" --json
1agent dashboard --journal-dir "$HOME/.1a/journals" \
  --alert-log "$HOME/.1a/schedule-alerts.jsonl" --out "$HOME/.1a/dashboard.html"
```

汇总最近运行、stop_reason 分布、计划任务和下一次运行时间；不调用模型，适合作为 daemon 的每日任务。

成本与延迟：

```bash
1agent cost --journal-dir "$HOME/.1a/journals"
1agent cost --journal-dir "$HOME/.1a/journals" --json
```

按 provider/model 汇总 token、成本与 p50/p95 延迟；数据来自 run journal，未上报 usage 的调用按 0 计。

检查 provider 配置：

```bash
1agent providers --config configs/example.json --check
1agent providers --config configs/example.json --probe --allow-network
```

`--probe` 会发起网络请求；只在你信任的端点上使用。  
配置示例见 `configs/example.json`；凭据字段使用 `api_key_env` 引用环境变量。

## 4. TUI 基础

| 按键 | 作用 |
|---|---|
| `Enter` | 提交任务或执行命令 |
| `Tab` | 补全 `/` 命令 |
| `↑ / ↓` | 选择命令/菜单，或滚动 |
| `Esc` | 关闭面板/抽屉，回到对话 |
| `Ctrl+T` | 打开对话模式菜单 |
| `Ctrl+K` | 打开 `/` 命令抽屉 |
| `Ctrl+Q` | 请求退出；`Enter` 确认，`Esc` 取消 |
| `Ctrl+C` | 复制选中文本 |

> 本地 CLI/TUI 默认允许文件写入和终端执行；`--read-only` 可关闭。最终是否执行仍由 Authority/Policy 和 Workshop 决定。网络仍需 `--allow-network`。

运行中，输入框上方显示 `Working... 执行时间 · ↑输入 ↓输出 · cache 命中率`。token 和缓存数据来自 provider usage；未上报缓存时显示 `cache —`。

裸 `1agent tui` 仍启动新 session；退出后 banner 会显示当前会话标题和 `--session <id>` 继续命令。

输入以 `/` 开头时打开命令抽屉：

- 空查询按 `最近 / 会话 / 思考 / 证据 / 模型与权限 / 总控` 分类；
- 最近三次成功执行的命令置顶；
- 不可用命令会灰显并标注原因；
- `Tab` 补全参数命令，例如 `/frame `；
- `Enter` 对不完整命令只补全，第二次 Enter 才执行；
- `Esc` 关闭抽屉但保留输入。

### 常用命令

| 命令 | 用途 |
|---|---|
| `/help` | 当前作用域和可用命令 |
| `/status` | 运行状态、宿主、预算 |
| `/frame` | 整理事实、假设、约束、未知、验收标准 |
| `/challenge` | 红队审查计划/结论/假设/证据 |
| `/evidence` | claim / evidence / unknown / conflict |
| `/audit` | 审计当前 run 或 Master journal |
| `/report` | 任务或生产线报告 |
| `/mode` | 切换对话模式 |
| `/model` | 查看/选择下一次运行的模型路由 |
| `/permissions` | 查看/调整下一次运行权限 |
| `/session` | 会话生命周期 |
| `/workspace` | 工作目录、文件、diff、worktree |
| `/context` | 压缩上下文或生成摘要 |
| `/preset` | 可复用任务模板 |
| `/tools` | ToolRegistry、schema、权限、风险 |
| `/export` | 导出会话、报告或证据 |
| `/copy` | 复制最近回答 |
| `/cost` | token 与成本 |
| `/config` | TUI 配置 |
| `/reframe` | 判断目的/手段，必要时追问 |

破坏性命令（`/clear`、非 dry-run 的 `/dispatch`、`/control cancel`、`/control takeover`）需要第二次 Enter 确认；其他输入会取消确认。

> Master/Workshop 命令只在 Workshop Master 绑定下出现；standalone 模式不会显示。

## 5. Headless 运行

```bash
# 离线确定性 provider
1agent run --task "给出结论、证据和翻转条件"

# 结构化 JSON
1agent run --task "..." --json

# 限定允许访问的根目录（读写共用）
1agent run --read-root "$HOME/project" --task "阅读并总结项目结构"

# 只读运行（关闭写入和终端执行）
1agent run --read-only --task "审计项目但不改任何文件"

# 使用配置文件 + 网络
1agent run --config configs/example.json --allow-network --task "..."
```

常用限制参数：

| 参数 | 默认 | 作用 |
|---|---|---|
| `--max-steps` | 6 | 最大推理/工具步数 |
| `--max-tool-calls` | 8 | 最大工具调用数 |
| `--timeout-seconds` | 120 | 单次运行超时 |
| `--max-context-chars` | 100000 | 上下文字符预算 |
| `--read-root` | 当前目录 | 允许读写的根目录，可重复 |
| `--allow-network` | 关闭 | 允许 provider 网络访问 |
| `--read-only` | 关闭 | 关闭文件写入和终端执行 |
| `--allow-write` | 兼容保留 | 默认已开启；与 `--read-only` 互斥 |

### 执行与文件工具（实验）

默认开启文件写入和终端执行；`--read-only` 或 TUI `/permissions write off` 可关闭：

```bash
1agent run --task "在终端执行 echo hello 并报告输出"
# TUI 内可用 /permissions write off 临时切回只读
```

可用工具：

| 工具 | 作用 |
|---|---|
| `read_file` | 读取读根内的 UTF-8 文本文件；拒绝符号链接与越界路径 |
| `read_pdf` | 调用系统 `pdftotext` 抽取 PDF 文本；受读根、符号链接和控制字符约束 |
| `search_files` | 在读根内搜索文本；跳过不安全文件 |
| `fetch_url` | 只读抓取公网 HTTPS 页面并抽取正文；需要 `--allow-network` |
| `web_search` | 公网搜索，返回最多 8 条标题/URL/摘要；Bing RSS 尽力而为，可能限流 |
| `browser_read` | 无头 Chrome 渲染公网 HTTPS 页面并抽取正文；只读，不支持点击/输入 |
| `spawn_agent` | 启动有界子 Agent；独立 journal/工作区/预算，深度上限 2 |
| `skill_list` | 列出已注册技能 |
| `skill_eval` | 用静态 held-out suite 评测技能版本 |
| `skill_register` | 注册新的不可变技能版本（写权限） |
| `skill_activate` | 激活技能版本；带 suite 时先过评测门（写权限） |
| `skill_rollback` | 回滚到旧技能版本（写权限） |
| `delegation_list` | 列出委托 profile 与撤销记录 |
| `delegation_register` | 注册 actor/domain 委托 profile（写权限） |
| `delegation_revoke` | 撤销委托（写权限） |
| `write_file` | 原子写入完整文件；默认拒绝覆盖，父目录必须存在 |
| `edit_file` | 精确替换文件文本；默认要求唯一匹配，支持 `replace_all` |
| `terminal_create` | 创建会话，返回 `session_id` |
| `terminal_write` | 输入文本或命令；默认追加回车 |
| `terminal_output` | 读取有界输出；ANSI 与控制字符会被清洗 |
| `terminal_exec` | 非交互执行命令，返回退出码与有界输出 |
| `terminal_destroy` | 销毁会话，释放 PTY |

依赖 `skynet-term-sessiond` 和 PATH 上的 `skynet-workshop-session`。需要人工查看或输入时，在另一个终端运行 `skynet-workshop-session attach <session_id>`。终端工具风险为 high；命令以当前用户权限执行，输出会写入 run journal，不要把密钥打印到终端。文件工具风险为 medium：路径必须在允许根内，拒绝符号链接，覆盖需显式 `overwrite=true`，写入采用同目录临时文件原子替换。

`fetch_url` 只允许公网 `https://` 地址，拒绝私网/loopback/link-local、带凭据的 URL、非文本内容、超过 2 MiB 的响应；重定向最多 5 次，每一跳都会重新校验并固定到公网地址。

平台支持：

| 平台 | 终端工具 |
|---|---|
| Linux x86_64 | 已验证，需要 `skynet-term-sessiond` 与 `skynet-workshop-session` |
| macOS | 未验证；1Agent 核心可运行，终端工具等待 Workshop daemon/GUI 支持 |
| Windows | 不发布、不支持；等待 ConPTY 与服务层实现 |

### MCP 工具（实验）

通过 stdio 连接 MCP server，把远端工具映射为 `mcp_<server>_<tool>`：

```bash
1agent run --mcp-config configs/mcp.example.json --task "列出可用的 MCP 工具"
1agent tui --mcp-config configs/mcp.example.json
```

配置格式：

```json
{"servers":[{"name":"filesystem","command":["npx","-y","@modelcontextprotocol/server-filesystem","/home/skynet"],"env":{}}]}
```

只支持 stdio；HTTP/SSE 和工具市场未实现。MCP 工具默认按写工具处理，只有 server 声明 `readOnlyHint: true` 才不要求写权限；`destructiveHint: true` 标记为 high risk。输入 schema 由 MCP server 校验，1Agent 只做进程、权限、超时和 journal 边界。

### 定时任务与 daemon（实验）

```bash
1agent schedule add --name daily --every-seconds 86400 --task "总结今天的 workspace 变化"
1agent schedule add --name on-change --every-seconds 3600 --trigger-file /tmp/input.txt --task "..."
1agent schedule add --name routine --every-seconds 86400 --routine-skill release-check@1.0.0 --task "检查发布"
1agent schedule list
1agent schedule health      # ok / overdue / failing，默认 300 秒宽限
1agent schedule remove <schedule-id>
1agent daemon --once       # 跑一次到期的任务
1agent daemon --alert-log "$HOME/.1a/schedule-alerts.jsonl" \
  --alert-webhook https://example.com/hook   # 失败/超时告警
1agent daemon              # 常驻，按 poll-seconds 检查
```

状态默认写入 `$HOME/.1a/schedules.json`；daemon 用 pid 锁防止重复启动。每个到期任务作为独立的 `1agent run` 子进程执行，使用自己的 journal 目录，并继承 `AGENT_*` provider 环境。任务可分别设置 `--read-only`、`--allow-network` 和工作区。`--trigger-file` 会在文件 mtime 变化时提前触发一次；interval 与 trigger 可同时使用。停机期间已到期的任务会在 daemon 重启后补跑一次。`--routine-skill NAME@VERSION` 只接受已激活的精确版本，并把技能正文注入任务；它不授予额外权限。任务失败或超时时，daemon 可写 `schedule_run_failed` 告警日志并发送 webhook；升级阶梯、跨重启去重和跨机器调度仍未实现。

技能包可通过 `1agent skill export/import` 分发；导入必须提供 bundle 的 SHA-256，已存在版本会跳过。需要来源认证时：

```bash
1agent skill keygen --private-key signing.key --public-key signing.pub
1agent skill export --store skills.jsonl --out bundle.json --signing-key signing.key
1agent skill import --store skills.jsonl --bundle bundle.json \
  --sha256 <bundle-sha256> --public-key signing.pub
```

私钥文件必须是 0600；带签名的包若未提供公钥会被拒绝，提供公钥但包未签名也会被拒绝。市场索引/撤销列表尚未实现。

## 6. 会话与工作区

```bash
1agent tui --session <session-id>
1agent tui --journal-dir .journal
```

TUI 的 `/workspace` 可以：

- 查看工作区概览、文件、diff；
- 切换授权根目录；
- 引用路径；
- 初始化 `AGENTS.md`；
- 管理 git worktree。

工作区路径必须位于授权 `roots` 内；越界路径会被拒绝。

## 7. 导出、恢复与迁移

```bash
# 导出（不包含密钥材料）
1agent export --output "$HOME/1agent-export"

# 恢复
1agent restore --bundle "$HOME/1agent-export"

# 检查/升级 home
1agent upgrade --check
1agent upgrade
1agent upgrade --rollback
```

导出包带有 SHA-256 清单；恢复会校验内容。  
密钥备份是单独的本地机制：

```bash
1agent key escrow --output "$HOME/1agent-keys.escrow.json" \
  --recovery-key "$HOME/1agent-recovery.key"
1agent key restore --escrow "$HOME/1agent-keys.escrow.json" \
  --recovery-key "$HOME/1agent-recovery.key"
```

把 escrow 和 recovery key 分开放；这不是 HSM/quorum 托管。

## 8. 卸载

```bash
# 保留数据
1agent uninstall

# 删除数据（危险，必须确认）
1agent uninstall --purge --yes
```

卸载二进制本身不会自动删除 `$HOME/.1a`，除非使用 `--purge`。

## 9. 本地服务与集成

### stdio host protocol

```bash
1agent serve
```

适合本地进程集成；协议细节见仓库 `docs/io-contracts.md`。

### Surface HTTP（loopback）

```bash
export SURFACE_TOKEN="$(openssl rand -hex 32)"
1agent surface serve \
  --addr 127.0.0.1:8788 \
  --store "$HOME/.1a/task-graph.jsonl" \
  --token "$SURFACE_TOKEN"
```

- 默认只绑定 loopback；
- 需要 Bearer token；
- 不是公网 SaaS，不是多租户服务；
- API 子集和权限边界见仓库 `docs/` 与 `surface` 帮助。

## 10. 故障排查

| 现象 | 处理 |
|---|---|
| `1agent: command not found` | 确认 `$HOME/.local/bin` 在 `PATH` |
| provider 凭据未就绪 | `1agent providers --config configs/example.json --check` |
| 网络请求被拒绝 | 显式加 `--allow-network`，并确认端点可信 |
| 文件读取被拒绝 | 使用 `--read-root` 指向授权目录；TUI 在 `/workspace` 中切换 |
| TUI 显示异常 | 使用真实终端；当前只保证 Linux x86_64 |
| doctor 报 journal/home 问题 | 先停止写入，再运行 `1agent doctor`；不要手工改 hash 链 |
| 忘记恢复密钥 | 没有恢复密钥无法解密 escrow；这是设计边界 |

## 11. 当前限制

- 没有浏览器、`web_fetch`、MCP、通用子 Agent；
- 没有完整 daemon/Routine/Trigger；
- Workshop 执行仍是研发/联调能力；
- Windows 未测试且不发布；macOS/Android/iOS 后续发布；
- 不是生产稳定版，接口和数据格式仍可能变化。

---

[返回 README](../README.md) · [产品功能](features.md) · [配套设施](ecosystem.md) · [发布状态](status.md)
