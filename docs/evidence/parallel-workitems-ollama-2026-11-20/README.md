# 并行 WorkItem + Ollama 证据（2026-11-20）

Status: **passed on this machine**  
Baseline: `1.0.0-rc.12`, branch `1.1-dev`, HEAD `341c6b2` + Batch 1 working tree  
Provider: local Ollama `qwen3-8b-16k` (`http://127.0.0.1:11434/v1`)  
Trace: `parallel-ollama-2026-11-20c`

## 测试设计

一个 Goal 拆成 **3 个互不依赖的 WorkItem**：

| WorkItem | 目标 | Workspace |
|---|---|---|
| `alpha` | 用 `write_file` 写 `result.txt`，内容 `alpha` | `/tmp/1agent-parallel-workitems/alpha` |
| `beta` | 用 `write_file` 写 `result.txt`，内容 `beta` | `/tmp/1agent-parallel-workitems/beta` |
| `gamma` | 用 `write_file` 写 `result.txt`，内容 `gamma` | `/tmp/1agent-parallel-workitems/gamma` |

`max_parallel = 3`；三个 WorkItem 分配給三个独立的 Ollama runtime profile
（`ollama-alpha` / `ollama-beta` / `ollama-gamma`），各自独立 workspace。
这不是“多个无关任务”，也不是“同 workspace 并发编辑”；它测的是**一个任务拆成多个
可并行 WorkItem，各自产出 artifact，再由 Master 独立验收**。

## 环境说明

- Workshop 0.11.1 的默认 sessiond 已有 128 个活动会话，`skynet-term-agent-run`
  返回 `session_limit`。本次按 `skynet-term/scripts/1agent-integration-smoke.sh`
  的方式启动**私有 sessiond**（独立 `SKYTERM_SESSION_DIR` / `SKYTERM_SESSION_SOCKET`
  / `SKYTERM_AGENT_DIR` / `SKYTERM_PRODUCTION_DIR`），未改动现有 128 个会话。
- 安装缺少的兼容别名：`skynet-workshop-lease` → `skynet-term-lease`
  （放在私有 PATH 目录，不改冻结的 `skynet-term`）。
- runtime wrapper：`scripts/ollama-workitem-runtime.sh`，把 Workshop 追加的 task
  文本转成 `1agent run --config configs/ollama-local.json --tool-query write_file
  --task "$task" --read-root "$PWD"`。运行后用 repo-relative 路径再次验证了单个
  WorkItem 能写出 artifact。

## 结果

| Task | Agent | Session | Start (unix ms) | End (unix ms) | Artifact |
|---|---|---|---|---|---|
| `alpha` | `ollama-alpha` | `term-1795140727344-2069514-2` | 1795140727322 | 1795140783989 | `alpha` |
| `beta` | `ollama-beta` | `term-1795140727343-2069514-1` | 1795140727321 | 1795140777772 | `beta` |
| `gamma` | `ollama-gamma` | `term-1795140727344-2069514-3` | 1795140727321 | 1795140770310 | `gamma` |

- 三次 lease 决策在 `1795140727314/7316/7318`，三个 session 创建于
  `1795140727345/7346/7346`，三个执行区间重叠 → **真实并行派发**。
- 每个 child 的 1Agent journal 只有 **1 次 `write_file` 调用**，参数正确，
  observation `ok`；artifact 内容分别为 `alpha` / `beta` / `gamma`。
- 三个 child journal：14 events，`audit ok`，`replay ok`。1Agent 最终
  `terminal_reason=needs_evidence`（本地 8B 的最终 answer 没有形成结构化证据），
  但工具调用和 artifact 都是真实的；**执行成功不等于验收通过**。
- Master 三个 receipt 均 `completed`，三个 artifact check 均 `verified`，
  `verified_by=operator` 与执行 Agent 不同；`goal_accepted=true`；
  Master journal audit `15 events / 0 issues / ok`。
- Artifact SHA-256 见 `artifacts.sha256`；child journal SHA-256 见
  `child-journals.sha256`（原始 journal 在 `~/.1a/journals/`，不提交）。

## 这证明了什么

- 1Agent Master 能把一个 Goal 拆成 3 个独立 WorkItem，并行租约、并行派发；
- 每个 WorkItem 运行在独立 workspace，read root 与 artifact 都受限；
- Workshop 适配器、production lease、idempotency key、session 回执可用；
- Master 的验收不是信模型自述：它重新检查 artifact 的存在性、regular-file、
  路径约束和 SHA-256；验收人必须不同于执行 Agent；
- 只有三个 WorkItem 全部 `verified` 后，`master accept` 才接受 Goal；
- 本地 Ollama 8B 可以驱动三个并行 WorkItem（Ollama 服务端可能串行生成，
  但 Master/Workshop 的派发和会话是并行的）。

## 这没有证明什么

- **同 workspace 并发写/合并**：本次三个 WorkItem 用独立 workspace，避开了
  写冲突；共享文件编辑仍需要 MA-017 的 file claim/lease 或合并协议。
- 本地 8B 的任务质量：child answer 是 `needs_evidence`，本证据只证明编排/验收
  机制，不证明模型质量，也不等同 M1。
- 超 128 session 的规模、跨进程 fanout、timeout/cancel/补偿路径、TUI/GUI。
- WorkItem 之间的依赖 DAG；本次三个 WorkItem 全部无依赖。

## 复现

```bash
W=/tmp/1agent-parallel-workitems/workshop
mkdir -p "$W"/{sessions,agents,production,run,config,logs,state}
export SKYTERM_SESSION_DIR="$W/sessions" \
  SKYTERM_SESSION_SOCKET="$W/run/sessiond.sock" \
  SKYTERM_AGENT_DIR="$W/agents" \
  SKYTERM_PRODUCTION_DIR="$W/production" \
  SKYTERM_CAPABILITY_POLICY="$W/config/capabilities.json" \
  XDG_STATE_HOME="$W/state" XDG_CONFIG_HOME="$W/config" XDG_RUNTIME_DIR="$W/run"
nohup skynet-term-sessiond > "$W/logs/sessiond.log" 2>&1 &

mkdir -p /tmp/1agent-parallel-workitems/bin
ln -sf /home/skynet/skynet-term/target/release/skynet-term-lease \
  /tmp/1agent-parallel-workitems/bin/skynet-workshop-lease
export PATH=/tmp/1agent-parallel-workitems/bin:$PATH

1agent master init \
  --plan docs/evidence/parallel-workitems-ollama-2026-11-20/plan.json \
  --agents docs/evidence/parallel-workitems-ollama-2026-11-20/agents.json \
  --trace-id parallel-ollama-2026-11-20c \
  --state-root /tmp/1agent-parallel-workitems/.master3

1agent master approve --trace-id parallel-ollama-2026-11-20c \
  --state-root /tmp/1agent-parallel-workitems/.master3 --actor user

1agent master run --trace-id parallel-ollama-2026-11-20c \
  --state-root /tmp/1agent-parallel-workitems/.master3 \
  --runtime-profiles docs/evidence/parallel-workitems-ollama-2026-11-20/agents.json \
  --workshop-prefix skynet-workshop --no-gui
```

然后对 `alpha` / `beta` / `gamma` 分别执行 `master verify --input verify-*.json`，
最后 `master accept`。

## 文件

- `plan.json` / `agents.json` — 三个 WorkItem 与三个 runtime profile。
- `run3.json` — Master 派发结果、receipts、state。
- `status.json` — 最终状态（三个 `verified`，`goal_accepted=true`）。
- `master-audit.json` — Master journal audit（15 events，0 issues）。
- `verify-*.json` / `verify-*.out.json` — 独立验收输入/结果。
- `accept.json` — Goal 接受记录。
- `sessions.json` — 私有 sessiond 的三个 session。
- `child-*.output.json` — 三个 1Agent child 的 JSON 输出（含 run_id/journal_path）。
- `child-audit.txt` — 三个 child journal 的 audit/replay。
- `artifacts.sha256` / `child-journals.sha256` — 内容摘要。
