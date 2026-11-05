# 002 架构与边界

Status: normative  
Depends on: `000-thesis.md`、`001-domain.md`  
Owns: 分层、组件、依赖方向、部署拓扑、API 边界  
Does not own: 权限细节（`003`）、记忆算法（`004`）、界面（`007`）

## 1. 架构原则

1. **内核与界面分离**：内核不知道 Web/TUI/IM 的存在。
2. **内核与 provider 分离**：模型只是端口，换厂商不改领域。
3. **事件溯源**：状态从 journal 重建；数据库是索引，不是事实来源。
4. **端口与适配器**：Source、Channel、Model、Sandbox、Store 都是端口。
5. **单一写入者**：每个聚合只有一个写入路径，避免并发写冲突。
6. **可停止**：所有长任务都有取消点；不可取消的操作必须显式声明。
7. **默认隔离**：Bot、Workspace、Ticket、Tenant 默认不共享状态。
8. **可观测**：每个动作有 trace/run/ticket id，能串起来。

## 2. 分层

```text
┌──────────────────────────── Surfaces ────────────────────────────┐
│ Web（主） · TUI · IM · Voice · HTTP API                          │
└───────────────────────────────┬──────────────────────────────────┘
                                │ 只通过 API/DTO，不直接读内核存储
┌─────────────────────────── Bot Runtime ──────────────────────────┐
│ Bot 身份 · 人格 · AuthorityProfile · Workspace · MemoryScope      │
│ SkillSet · Routines · Channels · 会话路由                        │
└───────────────────────────────┬──────────────────────────────────┘
                                │ 领域调用
┌──────────────────────────── 1A Kernel ───────────────────────────┐
│ EventBus · Journal · Gate · Eval · Router · TicketStore          │
│ AssignmentManager · WorkerRuntime · ToolRegistry · PolicyEngine  │
│ MemoryService · SkillService · ModelGateway · UsageLedger · Audit│
└───────────────────────────────┬──────────────────────────────────┘
                                │ 端口
┌──────────────────────────── Infrastructure ──────────────────────┐
│ Workshop/Sandbox · Store · Queue · SecretStore · Gateways · Obs  │
└──────────────────────────────────────────────────────────────────┘
```

依赖方向**只能向下**。禁止：

- 内核 import Web/TUI 类型；
- 内核直接调用 provider SDK（必须经 `ModelGateway`）；
- Surface 直接读 journal 文件（必须经 API）；
- Worker 直接写 Ticket 状态（必须经 Report + 状态机）。

## 3. 内核组件

| 组件 | 职责 | 不负责 |
|---|---|---|
| `EventBus` | 接收 Trigger、内部事件、定时器；投递到订阅者 | 业务判断 |
| `Journal` | append-only 事件、哈希链、重放 | 查询优化 |
| `Gate` | 确定性准入：来源、身份、白名单、去重、预算 | 语义判断 |
| `Eval` | Gate + 模型评估 → Decision | 执行 |
| `Router` | 把接受的 Ticket 分配给 Worker；不做技术决策 | 写代码/跑命令 |
| `TicketStore` | Ticket/Mandate/Assignment 状态机 | 执行 |
| `WorkerRuntime` | 启动/停止 Worker；隔离目录；心跳 | 业务策略 |
| `ToolRegistry` | 工具 schema、权限、超时、输出边界 | 模型调用 |
| `PolicyEngine` | 策略版本、准入表、执行策略、审批规则 | 具体动作 |
| `MemoryService` | 四层记忆的写入/检索/遗忘 | 授权 |
| `SkillService` | 技能版本、加载、评测、发布 | 执行 |
| `ModelGateway` | provider 路由、重试、成本、流式 | 领域状态 |
| `UsageLedger` | token/成本/时延账本 | 计费决策 |
| `Audit` | 重放、校验、收据 | 修复 |

## 4. Bot Runtime

一个 Bot 是一个长期实体，不是会话。

```rust
struct Bot {
    id: BotId,
    owner: OwnerId,
    name: String,
    persona: Persona,          // SOUL/风格/语言
    model_policy: ModelPolicy, // router/worker 模型偏好
    authority: AuthorityProfile,
    workspace: WorkspaceId,
    memory_scope: MemoryScope,
    skills: Vec<SkillId>,
    routines: Vec<RoutineId>,
    channels: Vec<ChannelId>,
}
```

规则：

- Bot 的 `authority` 不得大于 Owner 授予的上限。
- Bot 的记忆、技能、工作区默认私有；共享必须显式。
- Bot 可以有多个 Channel，但对外只有一个身份。
- Bot 可以有多个 Worker，但 Worker 不直接对 Owner 说话。

## 5. 数据存储

| 存储 | 内容 | 事实来源 | 备注 |
|---|---|---|---|
| Journal | 所有领域事件 | ✅ | append-only，哈希链 |
| State DB | Ticket/Assignment/Memory 索引 | ❌ | 可从 journal 重建 |
| Artifact Store | 文件/截图/PR 引用 | ❌ | 内容寻址，带摘要 |
| Vector/Graph Index | 语义/关联检索 | ❌ | 可重建，可替换 |
| Secret Store | API key/凭证 | — | 永不进 journal |
| Usage DB | token/成本/时延 | ❌ | 可从 journal 重建 |

原则：**Journal 是唯一事实来源；其他存储都是可重建的投影。**

## 6. API 边界

### 6.1 Surface API

- REST/JSON（命令与查询）+ SSE/WebSocket（流式事件）。
- 认证：本地 token / OAuth / 设备码；Bot 级 scope。
- 所有写操作带 `Idempotency-Key`。
- 所有响应带 `trace_id` / `ticket_id`（如适用）。
- 禁止 Surface 直接访问文件系统或 journal。

### 6.2 Host Protocol

沿用现有 `host` 协议 v3；新增 Bot/Ticket/Memory 事件时升级版本，保持向后兼容。

### 6.3 Tool/MCP

- 内部工具走 `ToolRegistry`。
- 外部工具走 MCP；MCP server 必须声明能力、权限、超时。
- MCP 工具默认不可信；首次使用需审批/白名单。

### 6.4 Model Gateway

- 统一 `complete` / `stream` / `tool_call` 接口。
- Provider 适配器只负责 wire format；不持有领域状态。
- 记录：provider、model、tokens、cost、latency、finish_reason。

## 7. 部署拓扑

### 7.1 本地单用户（首选）

```text
1A daemon（本机）
  ├─ SQLite + journal 文件
  ├─ 本地 sandbox（Linux/macOS/Windows）
  ├─ Web UI（localhost）
  └─ 可选：本地模型 / 远程 API
```

优点：隐私、低延迟、可离线；缺点：常驻需要守护进程/开机启动。

### 7.2 自托管单团队

```text
1A server（容器/VM）
  ├─ Postgres/对象存储
  ├─ 每 Ticket 一个 sandbox
  ├─ Web UI + IM 网关
  └─ 模型 API
```

### 7.3 云端多租户（后续）

只在 G2 之后考虑；需要租户隔离、配额、计费、合规。默认不做。

### 7.4 跨平台

| 平台 | 沙箱 | 备注 |
|---|---|---|
| Linux | bwrap/landlock/namespaces | 首选，CI 覆盖 |
| macOS | Seatbelt（sandbox-exec） | 需要最小 profile |
| Windows | Job Object + AppContainer/受限 token | 需要 PowerShell 适配 |

## 8. 非功能要求

| 维度 | 要求 |
|---|---|
| 持久性 | 已确认的 journal 事件不丢；fsync/批量策略明确 |
| 可用性 | 单机 daemon 崩溃后自动恢复；任务可 resume |
| 延迟 | Web 首屏 < 1s；流式首 token < 2s（取决于模型） |
| 吞吐 | 单机至少 4 个并行 Worker；可配置上限 |
| 安全 | 默认拒绝；最小权限；密钥不落盘 |
| 可观测 | 每个动作有 trace；关键指标可导出 |
| 成本 | 每任务预算；超预算停止并报告 |
| 可移植 | 核心不依赖 OS；平台差异在适配器 |

## 9. 与当前代码的映射

| 新组件 | 现有基础 | 动作 |
|---|---|---|
| Journal | `src/journal.rs` | 复用，扩展事件类型 |
| PolicyEngine | `PermissionSet` + config | 扩展为版本化策略 |
| ToolRegistry | `src/tools/mod.rs` | 增加 shell/write/network，沙箱化 |
| MemoryService | journal + experience | 新增 semantic/user model |
| SkillService | experience store | 新增 SKILL.md/版本/市场 |
| Router | Master planner/scheduler | 复用 Work Item/lease |
| WorkerRuntime | Workshop adapter | 增加本地 sandbox worker |
| ModelGateway | `src/provider.rs` | 增加流式/多 provider |
| Surfaces | `src/tui.rs` | 新增 Web；TUI 保留 |
| UsageLedger | `src/usage_ledger.rs` | 复用 |

## 10. 构建 vs 采购

**自己构建**：领域模型、授权、journal、Ticket/Mandate、审计、记忆治理、技能生命周期。

**复用/集成**：模型 provider、OS 沙箱原语、Web 框架、存储、消息网关、MCP。

**不要自己造**：LLM、向量数据库、密钥管理、身份认证协议、计费系统。

## 11. 架构决策记录

每个重大选择（存储、Web 框架、沙箱方案、消息队列、部署形态）必须开 ADR，
记录在 `010-governance.md` 的决策日志中。未记录的架构选择视为临时方案，不得依赖。
