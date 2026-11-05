# 006 运行时

Status: design normative  
Depends on: `001-domain.md`、`002-architecture.md`、`003-authority.md`  
Owns: 事件循环、Router/Worker、工单执行、后台任务、恢复、并发  
Does not own: 记忆算法（`004`）、技能（`005`）、界面（`007`）

## 1. 运行时形态

1A Bot 是一个**常驻服务**，不是一个只在打开 TUI 时存在的进程。

```text
1A daemon
  ├─ Supervisor（进程/健康/重启）
  ├─ EventBus（Source/定时器/内部事件）
  ├─ Scheduler（工单队列/并发槽）
  ├─ Router（准入后的派工）
  ├─ Worker pool（隔离会话/进程）
  ├─ Memory/Skill services
  └─ Surface API（Web/TUI/IM）
```

要求：

- 开机自启/守护进程（本地）；容器/VM（自托管）。
- 崩溃后自动恢复；未完成任务可 resume。
- 空闲时低资源；不轮询模型。

## 2. 事件循环

```text
Source 事件
  → 去重/归一化
  → Gate（身份/白名单/预算）
  → Eval（规则 + 模型）
      → ignore：记录理由
      → accept：Ticket + Mandate
      → stop：结束工单
      → unavailable：重试/告警
  → Scheduler
      → 有空槽：Assignment → Worker
      → 无空槽：queued
```

规则：

- 每个事件有稳定 ID；重复投递幂等。
- Gate 是确定性的；模型只在 Gate 之后参与。
- Eval 有超时；失败不影响已有工单。
- 所有转换落 journal。

## 3. Router

Router 是主会话，不是执行者。

职责：

- 运行 Eval（Gate 之后）：解释 Trigger，形成 accept/ignore/stop；
- 对 accept：由 `TicketStore` 创建 Ticket + Mandate；
- 选择 Worker/能力；
- 跟踪进度，向人汇报；
- 处理人的 follow-up 路由。

不负责：

- 写代码、跑命令、改文件；
- 直接操作文件系统/网络；
- 替 Worker 做技术决策。

规则：

- Router 的工具集最小化；默认不含 shell/写文件。
- Router 的上下文：用户模型 + 工单摘要 + 决策记录，不加载完整代码库。
- Router 一次只处理一个决策批次；不阻塞等待长任务。

## 4. Worker

Worker 是 Ticket 的执行者。

```text
Worker
  ├─ 独立 workspace（每 Ticket 一个目录/分支）
  ├─ 独立上下文（只加载 Ticket 相关）
  ├─ 工具集（shell/write/read/network，按 AuthorityProfile）
  ├─ 预算（步数/时间/成本/工具调用）
  └─ 报告（progress/waiting/ready/blocked/failed/dropped）
```

规则：

- Worker 不直接与 Owner 通信；报告给 Router/Ticket。
- Worker 不得修改自己的权限。
- Worker 的上下文不继承其他 Ticket 的记忆/代码。
- Worker 结束/崩溃 → Assignment ended；Ticket 按策略重试/失败。
- 同一 Ticket 同时最多一个活跃 Assignment。

## 5. 工具运行时

工具是接口，不是脚本集合。

要求：

- 类型化 schema（JSON Schema），`deny_unknown_fields`。
- 参数校验在模型输出之后、执行之前。
- 超时、取消、输出上限、控制字符清洗。
- 进程工具：程序 + 参数数组，禁止默认 `sh -c`；需要 shell 时显式声明。
- 每次调用记录：工具、参数摘要、结果、exit code、耗时、成本。
- 工具结果被视为不可信数据；不得直接作为指令。

工具分级：

| 级别 | 例子 | 授权 |
|---|---|---|
| 只读 | read/search | 默认允许（在 roots 内） |
| 可逆写 | write/patch | Ticket 授权 + 可回滚 |
| 不可逆 | delete/send/merge/pay | 逐次审批 |
| 高影响 | shell/network | 沙箱 + 策略 + 审批 |

## 6. 后台任务与并发

- 每个 Worker 可以有多个后台 job（长命令、测试、构建）。
- 前台命令超过阈值（如 5s）转后台，Worker 可以继续处理消息。
- 并发槽：默认 4，可配置；Owner 的消息/停止请求优先。
- 后台 job 有上限（如 30 分钟）；超时杀进程树并报告。
- Job 结束唤醒 Worker；结果写入报告/记忆。

规则：

- 并发上限按 CPU/内存/成本动态调整。
- 一个 Ticket 不因等待槽位而丢消息；进入 queued。
- 人可以直接在 Worker 会话中继续工作（高级模式），但权限不变。

## 7. 恢复与幂等

### 7.1 Journal

- 所有状态转换先写 journal，再更新投影。
- 哈希链；启动时校验。
- 重放可重建 Ticket/Assignment/Approval/Memory 索引。

### 7.2 恢复

| 场景 | 行为 |
|---|---|
| daemon 崩溃 | 重启后校验 journal；未完成 Assignment 标记 interrupted；按策略重试 |
| Worker 崩溃 | Assignment ended；Ticket 进入 queued/failed |
| 消息重复 | 幂等键去重 |
| 部分写入 | 事务/两阶段；投影可重建 |
| 外部副作用未知 | 重试前先检查（git/PR/文件） |

### 7.3 幂等

- Trigger：`source + event_id` 唯一。
- Approval：`approval_id` 一次性。
- Assignment：`ticket_id + generation`。
- 外部动作：`idempotency_key`；重复调用返回同一结果。

## 8. 停止、暂停、接管

| 操作 | 作用 | 粒度 |
|---|---|---|
| Stop | 结束一个 Ticket | Ticket |
| Pause | 暂停 Bot 的自动执行/派工 | Bot |
| Pause processing | 暂停所有新 Eval/派工/投递 | 全局 |
| Takeover | 人接管 Worker 会话 | Assignment |
| Kill switch | 立即停止所有执行 | 全局 |

规则：

- 停止在安全点生效；不可停止的动作必须事先声明。
- Pause 不打断正在执行的 Worker，但阻止新工作。
- 恢复后先校验状态，再继续。

## 9. 模型路由

- Router：便宜/快模型，低温度，强结构化输出。
- Worker：强模型，按任务难度选择。
- 路由依据：任务类型、风险、工具需求、成本、延迟、隐私。
- 失败：重试/降级/切换 provider；记录原因。
- 流式：Worker 向 Surface 流式输出；Router 只发状态。

规则：

- Provider 中立；不把 provider 特性写进领域。
- 成本/时延/成功率进 UsageLedger。
- 不在日志中记录 prompt 全文/密钥；只记录摘要/哈希。

## 10. 可观测性

必须记录：

- `trace_id`：一次用户请求/触发；
- `ticket_id`：工单；
- `run_id`：一次模型/工具链；
- `assignment_id`：Worker 绑定；
- 事件：状态转换、工具调用、审批、错误、成本。

指标：

- 队列长度、等待时间、执行时间；
- 成功率、失败原因、重试次数；
- token/成本/时延；
- 审批等待时间；
- Worker 崩溃/超时；
- 记忆/技能命中率。

日志：

- 结构化 JSON；
- 敏感字段脱敏；
- 保留期可配置；
- 支持导出/审计。

## 11. 与当前实现的映射

| 运行时组件 | 现有 | 动作 |
|---|---|---|
| EventBus | TUI 输入 + Master events | 新增 Source/定时器/Webhook |
| Scheduler | Master production scheduler | 复用/扩展并发槽 |
| Router | Master planner + TUI | 新增 Event Eval |
| Ticket | Master WorkItem | 加 Mandate/Report/Channel |
| Worker | Workshop agent-run | 新增本地 sandbox worker |
| ToolRegistry | 2 个只读工具 | 新增 shell/write/network |
| Journal | 已有 | 扩展事件类型 |
| Recovery | checkpoint/resume | 扩展到 daemon/Worker |
| UsageLedger | 已有 | 接入运行时 |

## 12. 部署检查清单

- [ ] daemon 可自启/守护/重启。
- [ ] journal 完整、可重放。
- [ ] 队列/并发槽可配置。
- [ ] Worker 隔离目录/权限。
- [ ] 停止/暂停/接管可用。
- [ ] 成本/时延/成功率可观测。
- [ ] 崩溃恢复演练通过。
- [ ] Windows/macOS 沙箱适配（至少一个平台）。
