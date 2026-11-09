
# 操作手册

[简体中文](manual.md) · [English](manual.en.md)

> 适用版本：**0.95.0 R1 release candidate**  
> 适用平台：**Linux x86_64**  
> Windows 版本尚未测试，暂不发布、不提供下载；macOS、Android、iOS 后续发布。

## 1. 安装

### 从发布包安装

```bash
tar -xzf 1agent-0.95.0-linux-x86_64.tar.gz
cd 1agent-0.95.0
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

> `--allow-write` 只表达本次运行的写权限意图；最终是否执行仍由 Authority/Policy 和 Workshop 决定。默认是只读。

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

# 带只读文件根目录
1agent run --read-root "$HOME/project" --task "阅读并总结项目结构"

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
| `--read-root` | 当前目录 | 只读文件根目录，可重复 |
| `--allow-network` | 关闭 | 允许 provider 网络访问 |

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
