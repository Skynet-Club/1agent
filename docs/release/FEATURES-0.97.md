# 1Agent 0.97.0 功能矩阵

状态：✅ 可用 · ◐ 部分可用 · 🧪 实验 · ✖ 未实现  
平台：Linux x86_64 已验证；Windows 内部构建、不发布；macOS 未支持

> “可用”指有本地测试/实机证据，不指生产稳定或第三方认证。
> 命令入口以 `1agent <command>` 为准；完整参数见 `docs/manual.md`。

## 1. 入口与运行

| 功能 | 状态 | 入口 | 权限/限制 |
|---|---|---|---|
| 交互式 TUI | ✅ | `1agent tui` | 本地终端；`Ctrl+Q` 两次退出 |
| Headless 运行 | ✅ | `1agent run --task "..." [--json]` | 与 TUI 共用 runtime/journal |
| 本地 HTTP surface | ◐ | `1agent serve` | 本地回环；token 保护；不是公网服务 |
| 初始化/诊断 | ✅ | `1agent init`、`1agent doctor` | 只创建/检查 `$HOME/.1a` |
| 导出/卸载 | ◐ | `1agent export`、`1agent uninstall` | 卸载默认保留数据；`--purge --yes` 才删除 |
| 会话继续 | ✅ | TUI banner 的 `--session`；`1agent resume` | resume 不重新审批未完成的副作用 |
| 流式输出 | ✅ | `1agent run --stream` | stdout 仍是最终 JSON；TTFT 进 journal |
| Python SDK（只读） | ◐ | `sdk/python/oneagent_client.py` | health/agent-card/trust-center/surface；非完整任务 SDK |

## 2. 任务、授权与审批

| 功能 | 状态 | 入口 | 权限/限制 |
|---|---|---|---|
| 任务图 / Ticket / Mandate | ◐ | `1agent task-graph` | 本地任务图；不是多租户工单系统 |
| Authority / Policy / ExecutionGate | ✅ | `--authority`、TUI `/permissions` | 只有 `Allow` 执行；`RequireApproval` 失败关闭 |
| 一次性/常驻审批 | ◐ | `1agent approval`、TUI 审批面板 | 本地审批；无外部 IdP |
| SideEffect 账本 / 补偿 | ◐ | `1agent repair-plan`、`repair-mark` | 可逆副作用必须有补偿 |
| 独立验证器 | ◐ | `1agent verification` | 执行者≠验证者；hidden 失败记 `hacked` |
| 自治档位 | ◐ | `1agent autonomy`、`autonomy-for` | 高风险/不可逆仍需审批 |
| 团队角色/交接 | ◐ | `1agent team-check`、`handoff` | 策略模块，不是 IdP |
| Clarify 追问 | ✅ | TUI `/clarify`、模型 `clarify` 工具 | 1–5 题批量；行/headless 不注册模型工具 |

## 3. 审计、回放与指标

| 功能 | 状态 | 入口 | 权限/限制 |
|---|---|---|---|
| 运行 journal | ✅ | 自动写入 `$HOME/.1a/journals` | 工具调用/观察配对；不是逐事件哈希链 |
| audit | ✅ | `1agent audit --journal PATH` | 校验配对、序列、答案结构 |
| replay | ✅ | `1agent replay --journal PATH` | 结构回放，不是模型重跑 |
| metrics | ✅ | `1agent metrics --journal-dir DIR` | 完成率/恢复/回放/工具/审计/成本/TTFT |
| dashboard | ◐ | `1agent dashboard` | 静态 HTML；非实时服务 |
| 决策收据 | ✅ | `1agent receipt`、`receipt-index` | 从 journal 投影 |
| 组织审计链 | ✅ | `1agent org-audit append/verify/seal` | SHA-256 链 + Ed25519 seal；本地索引 |
| OTel GenAI 导出 | ◐ | `1agent otel`、`otel-export` | OTLP/HTTP JSON；不是 OTel SDK/gRPC |
| 崩溃恢复 | ✅ | `1agent resume-status`、`resume` | 最后 checkpoint 后有工具调用的 journal 拒绝恢复 |
| 回滚演练 | ◐ | `docs/rollback-v0.97.md` | 0.96.1 不能读 0.97 的新 journal |

## 4. AG-23 智能闭环

| 功能 | 状态 | 入口 | 权限/限制 |
|---|---|---|---|
| `describe_self` | ✅ | 模型运行时工具 | 返回实时工具/权限/模型/预算；无静态能力目录 |
| `request_tools` | ✅ | 模型运行时工具 | 运行时校验未知/写/网络权限 |
| `task_need` 需求评估 | ✅ | `AgentAnswer.task_need` | freshness/locality/action/risk/ambiguity/budget |
| 证据再规划 | ✅ | 运行时 | 最多 2 次；仍不足转 `needs_evidence` |
| 策略记忆 | ✅ | `--experience-dir` | 只写 inert Strategy 草案，不存固定答案 |
| 工具预算最终作答 | ✅ | 运行时 | 到顶后一次无工具作答 + schema repair |
| 100 题评测套件 | ✅ | `1agent eval --suite evals/agent-loop-v1.json` | 固定 100 题；不是人工盲评 |

## 5. 工具面

| 工具 | 状态 | 权限 | 限制 |
|---|---|---|---|
| `read_file` | ✅ | 读 | 读根内；拒绝目录/隐藏控制字符 |
| `read_pdf` | ✅ | 读 | 只抽文本，不做 OCR/图片理解 |
| `search_files` | ✅ | 读 | 空 query 只列路径；不读内容 |
| `write_file` | ✅ | 写 | 写根内；副作用进 journal |
| `edit_file` | ✅ | 写 | 精确替换；写权限 |
| `terminal_create/write/output/exec/destroy` | ◐ | 写 | 真实 PTY；依赖 Workshop 底座时需其可用 |
| `fetch_url` | ✅ | 网络 | HTTPS/公网；2 MiB；拒绝私网/凭据 URL |
| `web_search` | ✅ | 网络 | Bing RSS；可能限流/本地化 |
| `browser_read` | ◐ | 网络 | 只渲染读取；Chrome 可跟随重定向，**不要用于不可信页面** |
| MCP stdio/HTTP | ◐ | 写 + 网络 | 所有 MCP 工具按写工具处理；不信任 `readOnlyHint` |
| `spawn_agent` | ✅ | 写 | 深度 2、扇出 8、独立 journal/工作区 |
| `skill_list/eval/register/activate/rollback` | ◐ | 读/写 | 技能包 SHA-256；静态扫描不是杀毒 |
| `delegation_list/register/revoke` | ◐ | 读/写 | 本地委派；非团队 IdP |

## 6. 记忆、用户模型与长期性

| 功能 | 状态 | 入口 | 限制 |
|---|---|---|---|
| 四层记忆 | ✅ | `1agent memory` | scope/taint/TTL；软/硬遗忘 |
| 记忆召回解释 | ✅ | `memory recall` | 输出可能含解密内容，勿写共享日志 |
| UserModel 2.0 | ✅ | `1agent user-model`、`onboard` | 偏好不是授权 |
| Goal / Commitment | ◐ | `onboard --goal/--commitment` | 本地 JSONL；无日历/邮件导入 |
| Attention / Digest | ◐ | `digest`、`away-digest` | 维护循环未全量 |
| ContextGraph | ◐ | 运行时/上下文视图 | 不是完整知识图谱 |
| Agent Inbox | ◐ | `1agent inbox`、`notify` | append-only；无多租户 |
| Trust Center | ◐ | `1agent trust-center` | 固定 `not_audited`；调用方报告≠独立审计 |
| Agent Card | ✅ | `1agent agent-card` | 可签名/验签；非 A2A registry |

## 7. 技能与进化

| 功能 | 状态 | 入口 | 限制 |
|---|---|---|---|
| SKILL.md 生命周期 | ◐ | `1agent skill` | 版本/扫描/评测/激活/回滚 |
| 技能捕获 | ◐ | `skill capture` | 启发式扫描，不是恶意软件沙箱 |
| 技能导出/导入 | ✅ | `skill export/import` | SHA-256 门禁 |
| Reflection / Lesson | ◐ | `1agent experience` | 人工激活 |
| Evolution Proposal / Ledger | ◐ | `1agent approval` | 不自批、可回滚 |
| 技能市场/远程签名 | ✖ | — | 未实现 |

## 8. 调度、守护与通知

| 功能 | 状态 | 入口 | 限制 |
|---|---|---|---|
| Schedule interval/trigger | ◐ | `1agent schedule add/list/remove` | 本地 JSON；不是 cron 替代 |
| Daemon 循环/重启补跑 | ◐ | `1agent daemon --loop` | pid 锁；无系统级守护 |
| Schedule health | ✅ | `1agent schedule health` | ok/overdue/failing |
| Watchdog 心跳/升级 | ◐ | `1agent watchdog` | 默认只建议；`--execute pause` 显式 opt-in |
| Webhook 告警 | ✅ | `--alert-webhook` | HTTPS/loopback；不跟随重定向 |
| Inbox 通知/升级 | ◐ | `inbox-notify`、`escalate` | 单向；不回收状态 |
| Chat bridge | ◐ | `1agent chat-bridge` | 文件式 spool；不是 IM 连接器 |

## 9. Provider、路由与成本

| 功能 | 状态 | 入口 | 限制 |
|---|---|---|---|
| OpenAI-compatible provider | ✅ | `--config`、`/model` | key 只在 env/0600 文件；不打印 |
| 本地 Ollama | ✅ | `configs/ollama-local.json` | 需本机 `ollama serve` |
| 确定性 fake provider | ✅ | 测试/评测 | 只用于确定性测试 |
| 模型路由 | ✅ | `1agent route-plan` | 延迟/成本/隐私约束 |
| 成本账本 | ◐ | `1agent cost` | 仅 provider 配置价格时有意义 |
| 预算/熔断 | ✅ | `--max-cost-micros`、`--max-tokens` | 超限 `BudgetExhausted` |
| Benchmark/评测 | ✅ | `benchmark-run`、`eval` | 契约夹具≠模型质量 |
| Answer quality 看板 | ✅ | `answer-quality` | 只算调用方标签；不采集/不盲评 |

## 10. 安全、发布与平台

| 功能 | 状态 | 入口 | 限制 |
|---|---|---|---|
| `--read-only` | ✅ | `run`/`tui` | 关闭写/终端；网络另需 `--allow-network` |
| Kill switch | ✅ | `$HOME/.1a/KILL` | 写工具失败关闭；不杀进程/不重启 |
| Authority/审批 | ✅ | `--authority`、`approval` | `RequireApproval` 失败关闭 |
| Provider 重定向 | ✅ | 内置 | 禁止跟随，避免 key 降级泄露 |
| Linux 包 | ✅ | `scripts/package.sh --linux-only` | 仅 Linux x86_64 |
| SHA256/provenance | ✅ | `SHA256SUMS`、`provenance verify` | provenance 未签名 |
| 可复现构建 | ◐ | `reproducible-build.sh` | 同机两次一致；跨构建机未证明 |
| Windows | ✖ | 内部构建 | 未测试、不发布 |
| macOS | ✖ | — | 未支持 |
| Computer-use | ✖ | — | 无点击/输入/截图 |
| 语音/图片理解 | ✖ | — | 无 |
| 多租户/组织级审计 | ✖ | — | 无 |
| Android/iOS | ✖ | — | 无 |
| 第三方审计/认证 | ✖ | — | 本地代码级评审≠认证 |
| 人工盲评 | ✖ | — | 当前 M19–M23 是 AI/批量标签 |
