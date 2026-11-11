
# 产品功能

[简体中文](features.md) · [English](features.en.md)

> 当前版本：**0.96.1 R1 release candidate**  
> 正式验证平台：**Linux x86_64**  
> 状态标记：**可用（RC）** / **基础可用** / **MVP** / **模块** / **实验性** / **研发中** / **未实现**

## 1. 一张图看当前能力

```text
用户
  │
  ├─ 1agent tui / run / serve
  │     │
  │     ├─ 任务与证据：Trigger / Thread / Ticket / Mandate / Approval / Report
  │     ├─ 记忆：Working / Episodic / Semantic / Procedural + UserModel
  │     ├─ 技能：SKILL.md / 版本 / 扫描 / 评测 / canary / rollback
  │     ├─ 长期性：Goal / Commitment / Attention / ContextGraph / Delegation / Evolution
  │     ├─ 模型：ProviderRegistry / Router / UsageLedger / Shadow
  │     └─ 审计：journal / receipt / replay / query / export
  │
  └─ 可选：Skynet Workshop
        └─ Session / 沙箱 / 租约 / PTY / Artifact / 恢复
```

1Agent 负责“决定做什么、为什么、是否完成”；Workshop 负责“在哪里、如何安全地执行”。

## 2. 当前可用能力

### 2.1 交互式 TUI（可用，RC）

- 单轮对话、任务运行、运行状态、活动提示；
- 裸启动创建新 session；退出需要明确确认，banner 给出 `--session` 继续命令；
- 运行中显示 `Working...`、执行时间、输入/输出 token 和缓存命中率；
- `/` 命令抽屉：分类、最近使用、模糊匹配、禁用原因；
- 对话模式：自动、直接回答、证据研究、决策分析、执行任务、诊断分析；
- `Ctrl+T` / `/mode` 打开模式菜单；
- `Ctrl+K` 打开命令抽屉；
- 输出 transcript 滚动、鼠标选择、复制；
- 运行中可取消；
- 完整命令表见 [MANUAL.md](manual.md) 和仓库 `docs/tui.md`。

### 2.2 Headless 运行（可用，RC）

```bash
1agent run --task "分析这段需求，指出目标和手段"
1agent run --task "..." --json
```

适合脚本、CI、批处理。与 TUI 共用同一套 runtime、journal、provider 和审计契约。

### 2.3 任务、授权与审批（可用，RC）

- 事件 → Thread → Gate → Eval → Ticket → Mandate → Assignment → Report；
- AuthorityProfile / PolicyEngine / ExecutionGate；
- 一次性 Approval、StandingApproval、SideEffect 账本；
- 重试、接管、取消、补偿、对账；
- 未授权动作在工具执行前拒绝；
- 独立验证器与 hack-verifiable 验收：执行者≠验证者；public 通过但 hidden 失败记为 `hacked`，不计 clean；输出 `hacked_resolved_bps`；
- Trust & Repair：副作用账本投影为 applied / compensated / failed / pending / needs_user；可逆副作用必须有补偿，否则失败关闭；`1agent away-digest` 查看“我不在时发生了什么”；
- 价值周报 v0：`1agent value-report` 汇总 completed / blocked / failed / cancelled、tokens/cost、工具调用/失败、alerts、open inbox 与 pending_decisions；只报可复现结果与每完成结果成本，不编造“省时”；
- 伙伴声音：operator / advisor / companion；`--voice-mode` 选择；只改变说话方式，不改变证据/授权/动作规则；companion 不谄媚、不角色扮演；
- Agent Inbox：notify / question / review 三类；append-only JSONL 快照；snooze、approve/reject/answer/dismiss 状态机；终端状态不可再迁移；重复 ID、空字段、无效 snooze 失败关闭；`1agent inbox add|list|resolve|snooze|sync`；attention/alerts 纯投影；`sync` 按 `item_id` 去重，默认 dry-run，`--apply` 写入，不覆盖已有状态；
- 自治档位策略：`1agent autonomy --level observe|suggest|act|auto --risk low|medium|high [--reversible] [--explicit-approval]`；observe/suggest 不行动；act 仅低风险可逆；auto 允许低/中风险可逆；高风险/不可逆需审批（显式批准除外）。偏好不是授权；
- Onboarding v0：`1agent onboard --voice ... --autonomy DOMAIN=LEVEL --boundary TEXT` 写入普通 memory 记录并回显 `UserModel`；非法值失败关闭；`--goal/--commitment` 可写入首个目标/承诺，随后 `inbox sync` 可投影；还不是访谈/上下文导入，memory 与 goal 写入非原子；
- TUI `/inbox`：列出 open / due-snoozed 项（kind/status/title/why_now）；支持 `/inbox resolve <id> <approved|rejected|answered|dismissed> [note]` 与 `/inbox snooze <id> <until_ms>`，与 CLI 共用解析/应用契约；
- Agent Card v0：`1agent agent-card [--json]` 输出 schema/version/platform/能力状态/限制；`GET /v0/agent-card` 以公开只读方式提供同一 payload；平台能力取平台注册表，unsupported/untested 不得标为 verified；本地未签名，尚非 A2A registry-signed 卡；
- Tool search：`1agent tools --query TEXT [--json]` 按名称/前缀/描述/参数名确定性排序；`1agent run --tool-query TEXT` 将模型可见工具限制为 top 8 + always-on（read_file/write_file/edit_file/terminal_exec），选择结果写入 journal；无 query 时保持全量；
- CUA 策略：`1agent cua-check` 按参数来源（plan/trusted/untrusted）、风险、可逆性与 planned branch 输出 allow/require_approval/deny；高风险 + untrusted → deny；planner 不看 untrusted；未规划 branch → deny。仅策略，无浏览器执行。

### 2.4 本地记忆（可用，RC）

- Working / Episodic / Semantic / Procedural 四层；
- scope、source、taint、TTL、valid_until；
- recall、soft forget、hard forget；
- 加密 payload（per-record DEK + per-scope KEK）；
- UserModel（偏好/边界/自治档位/声音/记忆同意；`1agent user-model` 可查看）；
- 记忆内容不写入 journal 正文，只记录必要元数据/摘要。

### 2.5 技能（可用，RC）

- SKILL.md、版本、索引、扫描；
- 静态 held-out 评测；
- 运行时 canary、晋级/回滚（模块与测试；与 Router 实时观测的接线仍在补齐）；
- 候选捕获与示范捕获接口（演示数据仅保留哈希）；
- 恶意技能/越权技能拒绝；
- `1agent skill scan --path SKILL.md [--json]`：注册前预检 frontmatter、控制字符、密钥/secret 标记与指令覆盖标记；不注册、不执行；
- `1agent skill verify --bundle PATH --sha256 HEX [--public-key PATH] [--json]`：与 import 共用 `verify_bundle`，校验 SHA-256、schema、签名、重复项、manifest hash 与扫描；不导入。

### 2.6 长期性能力（可用/部分）

| 能力 | 状态 | 当前边界 |
|---|---|---|
| Goal / Commitment | 可用（RC） | 状态机、持久化、Commitment→Ticket 关联；依赖/证据/Recurrence 仍在补齐 |
| Attention | 基础可用 | 确定性截止提醒、预算、去重、snooze；全量候选/Digest 研发中 |
| PersonalContextGraph | MVP | 从 Goal/Commitment/Ticket/Thread/Memory 只读重建；跨端/大规模压测未做 |
| DelegationProfile | 可用（模块） | observe/suggest/draft/ask/act-and-report；运行时接线和真实动作域仍有限 |
| 自我总结/进化 | 基础可用 | Reflection/Lesson、EvolutionProposal、Ledger、人工审批/回滚；自动候选研发中 |

### 2.7 模型路由（可用，RC）

- 确定性 provider + OpenAI-compatible provider；
- 能力硬过滤、质量/均衡/延迟/成本路由目标；
- provider 健康降级、cooldown、影子执行；
- 成本/延迟账本；
- 凭据通过环境变量或本地配置引用。

### 2.8 审计与证据（可用，RC）

- append-only hash-chained journal；
- replay、audit receipt、query、export；
- ActionLog；
- 工具调用/结果配对；
- `exit 0` 不自动升级为 `verified`；
- Artifact 存在性、路径约束、SHA-256 验证。

### 2.9 本地服务与集成（可用，RC 子集）

- `1agent serve`：stdio host protocol；
- `1agent surface serve`：loopback HTTP Surface API + token；
- 线程、工单、记忆、技能、审计、ActionLog、Goal/Commitment/Delegation/Evolution 的部分接口；
- 不是多租户 SaaS，不是公网服务。

### 2.10 产品化（可用，RC）

- 单一 runtime home：`$ONEAGENT_HOME` / `$HOME/.1a`；
- `init` / `doctor` / `export` / `restore` / `upgrade` / `uninstall`；
- key escrow / restore；
- Linux 可复现包、SHA256SUMS、provenance、安装脚本；Windows 仅内部构建，不发布；
- 平台能力注册表：`1agent platform [--json]` 输出 `verified` / `degraded` / `unsupported` / `untested`；macOS/Windows 当前均为 `untested`，不宣称支持；
- MIT 许可证和第三方 notices。

## 3. 明确未实现

| 能力 | 状态 | 说明 |
|---|---|---|
| 浏览器 / computer-use | **部分** | `browser_read` 可用无头 Chrome 渲染并抽取正文；点击、输入、截图和视觉理解未实现 |
| 通用子 Agent | **部分** | `spawn_agent` 有界子进程（深度 2、独立 journal/工作区）；Master→Worker 仍是独立路径 |
| MCP 客户端 | **部分** | stdio MCP：发现并调用工具；HTTP/SSE、OAuth 和工具市场未实现 |
| 常驻 daemon/Routine/Trigger | **部分** | interval、文件 trigger、重启补跑、`--routine-skill`、`schedule health`；失败/超时可写告警日志并发 webhook；升级阶梯和跨重启去重未实现 |
| 全量 Attention/Digest | **部分** | `digest` 汇总最近运行、计划任务、健康状态与告警计数；维护循环和升级阶梯未实现 |
| 自动进化/技能市场 | **部分** | 人工审批/回滚 + SHA-256 技能包分发；签名和远程市场索引未实现 |
| 语音/图片理解 | **未实现** | `read_pdf` 只能抽取 PDF 文本；语音、图片、扫描件 OCR 未实现 |
| 多租户/组织级审计 | **未实现** | 当前是单用户/本地优先 |
| Android/iOS App | **规划中** | 与 Bot 一起推进 |
| macOS 正式支持 | **规划中** | 当前只验证 Linux |
| Windows 正式支持 | **不发布（未测试）** | 仅内部构建；不提供下载，不作为支持平台 |

## 4. 典型使用场景

### 场景 A：重要决策分析

```bash
1agent run --task "我们应该现在迁移到新架构，还是等下一个季度？"
```

输出应包含：结论、事实、假设、未知、冲突、翻转条件，以及支撑证据。

### 场景 B：读代码 / 读文档

```bash
1agent run --read-root ~/project \
  --task "阅读这个项目的鉴权路径，列出风险最高的三个入口"
```

可用 `read_file`、`read_pdf`、`search_files`、`fetch_url`、`web_search`、`browser_read`，以及默认开启的 `write_file`、`edit_file` 和真实 PTY 终端工具（`--read-only` 可关闭写入）。

### 场景 C：交互式调查

```bash
1agent tui
```

在 TUI 里用 `/frame`、`/evidence`、`/audit`、`/report` 等命令整理问题和证据。

### 场景 D：受控执行（需要 Workshop）

本地 CLI/TUI 默认允许文件写入和终端执行；接入 Skynet Workshop 后，终端工具使用持久 PTY Session，并可执行、读取和销毁。Workshop 仍在研发/联调阶段，不应作为稳定生产依赖。

## 5. 当前版本的边界

- **不是生产稳定版**：R1 release candidate，接口和行为仍可能变化。
- **不是完整 Super-Agent**：computer-use 操作、语音/图片理解、自动 Watchdog 和移动端尚未完成。
- **不是 Bot**：没有人格、Web/IM/Voice 界面、Companion Contract。
- **不是多租户产品**：没有组织 IdP、租户隔离和第三方审计。
- **不是“任意网站操作”工具**：只能渲染并读取页面，不能点击或截图。
- **不是绕过授权**：本地写/执行默认开启；网络需 `--allow-network`，高风险动作仍受 Authority/Policy 和审批约束。

## 6. 相关文档

- [设计哲学](design-philosophy.md)
- [操作手册](manual.md)
- [配套设施](ecosystem.md)
- [发布状态与路线图](status.md)
- [Super-Agent 能力补齐计划](status.md)

---

[返回 README](../README.md)
