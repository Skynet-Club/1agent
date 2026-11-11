# 说法与证据对照表

这张表不是成绩单，是**让没证据的话显形**。

规则只有一条：左边每一句我们对外说过的话，右边必须有一条能重跑的证据；
没有就写 `无`，状态标 ❌。❌ 不丢人，❌ 假装成 ✅ 才丢人。

```bash
./scripts/verify-claims.sh          # 只查结构：有没有 ✅ 却没证据
./scripts/verify-claims.sh --run    # 真的去跑每一条证据
```

本表的结构检查已接入 `./scripts/verify.sh`：出现 ✅ 却没有可执行证据时，
全量门禁直接失败。

状态含义：

- ✅ 有可执行的证据，且已经跑过
- ⚠️ 有证据，但只能证明一部分
- ❌ 没有证据；这句话目前不该对外说

证据只允许四种写法，便于机器执行：

```text
test: <测试函数名>        → cargo test --offline <名字>（覆盖 lib 与集成测试）
cmd:  <shell 命令>        → 命令退出码必须为 0
file: <路径>              → 文件必须存在
无                        → 没有证据
```

## 产品承诺

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| 默认不重构：绝大多数请求按原样执行 | file: docs/evidence/reframing-heldout-v2/summary.json | ⚠️ | v3 在 held-out v2 上 20/21 保持沉默；但 v2 在 held-out v1 上仅 15/21，未达门槛 |
| 只问一个最值钱的问题 | cmd: python3 scripts/reframing/analyze_hypothesis_space.py | ⚠️ | 设计层证据（8 个构造案例），不是模型行为证据 |
| 替代方案必须列出新增代价 | 无 | ❌ | 契约字段未实现 |
| 决定权在用户，只读回答不得产生动作回执 | test: read_only_answer_rejects_action_receipts | ✅ | |
| 被拒绝后不再重复提议 | 无 | ❌ | 未实现 |
| 可查询历史决策收据 | 无 | ❌ | `/receipt` 未实现 |

## 工程不变量

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| 无写权限时 execute 模式不调用模型 | test: execute_mode_requires_authorization_without_calling_provider | ✅ | |
| 读文件不能越出允许的读根 | test: rejects_file_outside_allowed_root | ✅ | |
| 搜索不能越出读根，且拒绝符号链接起点 | test: search_rejects_symbolic_link_start_path | ✅ | |
| 工具观测被裁剪，隐藏控制字符被拒绝 | test: bounds_observation_output_and_rejects_hidden_controls | ✅ | |
| 工具调用与结果在 journal 中成对 | test: audit_detects_unpaired_tool_call | ✅ | |
| 远端凭据必须走 HTTPS | test: remote_bearer_credentials_require_https | ✅ | |
| 检查 provider 时不打印密钥 | test: provider_inspection_never_prints_secret_values | ✅ | |
| 写路径重叠的任务被串行化 | test: overlapping_write_roots_are_serialized | ✅ | |
| 平台分支只允许出现在展示适配器 | cmd: ! grep -rnF 'cfg(target_os' src --exclude=tui_terminal.rs | ✅ | 本日修正：原门禁依赖缺失的 rg，被静默跳过 |
| 技能可从捕获、评测、激活回滚到上一版本，失败候选不替换现役版本 | test: skill_lifecycle_capture_to_rollback_is_replayable | ✅ | 确定性集成场景；基于磁盘 journal、重启回放与审计收据；不代表真实示范语料 |
| UserModel 2.0 可从记忆重建；自治/声音/同意默认安全，遗忘记录即失效 | cmd: cargo test user_model --lib && cargo test --test user_model_cli | ✅ | 自治与同意是偏好，不是授权；Authority/Policy/Approval 仍决定执行 |
| 平台能力状态显式；`untested` 不宣称支持 | cmd: cargo test platform --lib && cargo test --test platform_cli | ✅ | Linux 状态图；macOS/Windows/other 全部 `untested`；不是 P-Gate 通过 |
| 独立验证器拒绝自验证、重复/缺失结果和无证据通过；public pass + hidden fail 记为 hacked | cmd: cargo test verification --lib && cargo test --test hack_verifiable | ✅ | 确定性契约夹具；不代表真实模型质量；隐藏测试需与公开测试分开维护 |
| 可逆副作用无补偿时失败关闭；不可逆/失败需人工；away digest 分区可见 | cmd: cargo test repair --lib && cargo test --test away_digest_cli | ✅ | 只规划补偿，不执行；执行仍需 Authority/Policy/Approval |
| 价值周报只报可复现结果与每完成结果成本，不编造“省时”；包含 open inbox 计数 | cmd: cargo test value_report --lib && cargo test --test value_report_cli | ✅ | mock journal；真实用户价值需后续测量基线；inbox 与 alerts 未去重 |
| 三种伙伴声音进入系统提示；无效模式失败关闭；不改变证据规则 | cmd: cargo test voice --lib && cargo test --test voice_mode_cli | ✅ | 契约级；盲评质量未做 |
| Inbox 快照可重放；终端状态不可迁移；重复 ID/空字段/无效 snooze 失败关闭；CLI 可操作；sync 去重且默认 dry-run | cmd: cargo test inbox --lib && cargo test --test inbox_cli && cargo test --test inbox_sync_cli | ✅ | CLI/投影/sync 已实现；TUI 与桌面/聊天入口未接 |
| 自治档位策略：observe/suggest 不行动；act 仅低风险可逆；auto 低/中风险可逆；高风险/不可逆需审批 | cmd: cargo test autonomy --lib && cargo test --test autonomy_cli | ✅ | 偏好不是授权；Authority/Policy/ExecutionGate 仍决定执行 |
| Onboarding 写入 memory 并可经 user-model 回读；非法值失败关闭；可 seed goal/commitment；`--interactive` 读 stdin | cmd: cargo test --test onboard_cli | ✅ | 脚本化访谈；自由追问/文档/日历导入未做；memory 与 goal 写入非原子 |
| TUI `/inbox` 列出并可 resolve/snooze；解析与应用契约与 CLI 一致 | cmd: cargo test inbox --lib && cargo test tui_commands --lib | ✅ | 只操作 inbox store；不绕过 Authority/Policy/Approval |
| Agent Card 列出能力状态与限制（含 verification store/team handoff/inbox notify/repair runbook/org audit/MCP HTTP）；unsupported/untested 不标为 verified；HTTP 公开只读提供 | cmd: cargo test agent_card --lib && cargo test --test agent_card_cli && cargo test http_surface --lib | ✅ | 签名可选；非 A2A registry-signed 卡 |
| skill scan 预检 frontmatter/控制字符/密钥/指令覆盖；不注册不执行 | cmd: cargo test skill --lib && cargo test --test skill_scan_cli | ✅ | 启发式扫描，不是 malware sandbox；签名/权限另算 |
| skill verify 与 import 共用 verify_bundle；校验 SHA/schema/签名/重复/manifest/扫描，不导入 | cmd: cargo test skill_bundle --lib && cargo test --test skill_verify_cli | ✅ | 证明完整性，不证明运行安全 |
| CUA 策略：untrusted high-risk deny；untrusted require_approval；planner 不看 untrusted；未规划 branch deny | cmd: cargo test cua --lib && cargo test --test cua_check_cli | ✅ | 仅策略，无浏览器执行/截图 |
| Cascade 模拟计算 affected/contained/propagated、containment_bps/depth/blast radius 并按策略门禁 | cmd: cargo test cascade --lib && cargo test --test cascade_check_cli | ✅ | 确定性模拟，不是真实多 Agent 运行 |
| Budget 硬上限：超限失败、80% 警告；每验证结果成本；超限非零退出；run 运行时累计并停止 | cmd: cargo test budget --lib && cargo test --test budget_check_cli && cargo test cost_budget_stops --lib | ✅ | 运行时按 provider-reported usage 累计；未覆盖所有 provider 计费差异 |
| OTel 投影生成 invoke_agent/chat/execute_tool spans，带 gen_ai 属性且确定性 | cmd: cargo test otel --lib && cargo test --test otel_cli | ✅ | JSON 投影，非完整 OTel SDK/exporter |
| Chat 适配契约：消息→TaskRequest；回复先结论+信任摘要+needs_approval+inbox_open，不泄露 schema；文件桥 task/reply/ingest spool（同 stem 跳过） | cmd: cargo test chat --lib && cargo test chat_bridge --lib && cargo test --test chat_bridge_cli | ✅ | 文件式适配器，非 Slack/Telegram/微信连接 |
| 团队角色策略：owner/approver/member/viewer × read/write/approve/manage + scope；未知 actor/越权/未授权拒绝；可选写入组织审计链 | cmd: cargo test team --lib && cargo test --test team_check_cli | ✅ | 策略模块，非 IdP；审计 append 与判定非原子 |
| Agent Card Ed25519 签名与信任公钥验签；私钥可写 0600 文件并从文件签名；篡改/错钥/未签名拒绝 | cmd: cargo test agent_card --lib && cargo test --test agent_card_cli && cargo test --test card_trust_cli | ✅ | 本地签名，非 A2A registry 发现/签名条目；`--sign-key` 仍会进 argv |
| 持久幂等记录：key+fingerprint→response append-only JSONL，sync 后确认，跨重启重放；冲突/半条记录处理 | cmd: cargo test idempotency --lib && cargo test --test idempotency_cli | ✅ | 去重原语；未接入所有外部写 |
| 带外 watchdog：daemon 每轮写心跳；缺失/过期/损坏非零退出 | cmd: cargo test watchdog --lib && cargo test --test watchdog_cli | ✅ | 只观察存活；不重启/不清理锁 |
| daemon 安静时段：窗口内跳过 due 任务、保持 due、窗口后运行 | cmd: cargo test --test daemon_quiet_cli | ✅ | 固定 UTC offset；DST 由调用方处理 |
| 自治安静时段：跨午夜窗口内主动 act/approval → defer 并报告剩余分钟；用户发起/显式批准不延后 | cmd: cargo test autonomy --lib && cargo test --test autonomy_cli | ✅ | 策略级；运行时调度/安静时段持久化未接线 |
| 误报预算：预算耗尽或误报率超限时非关键告警转 digest，critical 绕过 | cmd: cargo test alarm_budget --lib && cargo test --test alarm_budget_cli | ✅ | 策略函数；不发送/不持久化告警 |
| 升级阶梯：严格递增阈值取最高已到达台阶；resolved 不升级；宽限期不打扰 | cmd: cargo test escalation --lib && cargo test --test escalate_cli | ✅ | 只计算级别；不发送/不改状态 |
| 组织审计索引：actor/action/scope/decision 哈希链；改字段/换序/断链检测 | cmd: cargo test org_audit --lib && cargo test --test org_audit_cli | ✅ | 本地索引，非远程透明日志/IdP |
| MCP OAuth claims：audience/过期/未生效/最大 TTL/精确 scope；拒绝时不泄露 subject | cmd: cargo test mcp_auth --lib && cargo test --test mcp_token_cli | ✅ | 调用方必须先验签；不实现 issuer discovery/token exchange |
| MCP streamable HTTP：JSON/SSE 响应、Mcp-Session-Id、仅 loopback HTTP、token 只读环境变量；结果带 server/tool 审计元数据 | cmd: cargo test tools::mcp --lib | ✅ | 覆盖初始化/发现/调用；真实服务端互操作未验证 |
| 每验证结果成本：value-report 接收验证器计数或 verification store，输出 cost_per_verified_outcome 与窗口 verification summary | cmd: cargo test value_report --lib && cargo test --test value_report_cli && cargo test --test verification_store_cli | ✅ | store 只计 clean Verified；不从 completed 推断 |
| 验证记录库：append+sync、重复 id/自验证拒绝、summary/hacked bps、窗口计数、Hacked/Rejected 投影 inbox review | cmd: cargo test verification_store --lib && cargo test --test verification_store_cli | ✅ | 本地 JSONL；无跨机聚合 |
| Windows GNU 交叉编译：`-D warnings` 下 lib/bin 与 all-targets（含测试）cargo check 通过；`scripts/cross-check.sh` 强制该门 | cmd: ./scripts/cross-check.sh | ✅ | 仅编译；链接/运行/ConPTY/安装包未验证；macOS 被 Host C 工具链阻断 |
| Onboarding 文本导入：按行写入 memory，大小/行数/行长限制，非法 kind/文件拒绝 | cmd: cargo test --test onboard_cli | ✅ | 纯文本；不脱敏；非日历/邮件导入 |
| Inbox 接受交接：handoff:<id> approved 需要 store+actor，先接受再关闭；错 actor/终态拒绝 | cmd: cargo test --test handoff_inbox_cli | ✅ | 本地；非原子（接受后 inbox 写入失败可能部分完成） |
| 组织审计 seal：Ed25519 签 entries+head_hash；追加/改 head/错钥拒绝 | cmd: cargo test org_audit --lib && cargo test --test org_audit_seal_cli | ✅ | 依赖 seal 公钥的带外信任；非远程透明日志 |
| 每日摘要：运行/计划/健康/告警/inbox open/指定收件人 open handoffs/验证摘要 | cmd: cargo test --test digest_cli | ✅ | 汇总只读；不含维护循环/通知 |
| Repair runbook：compensate/manual/errors/clean，require-clean 非零 | cmd: cargo test repair --lib && cargo test --test repair_plan_cli | ✅ | 只计划；不执行补偿/不调用 Authority |
| 单向 Inbox 通知：open items → webhook，HTTPS/loopback、拒绝凭证、空队列不发送、`--state` watermark 去重、`--log` 回执 | cmd: cargo test notify --lib && cargo test --test inbox_notify_cli | ✅ | 只发不收；不改 item 状态；非完整聊天传输 |
| 信任披露清单：能力计数/限制/数据政策/评测披露/本地验证摘要；audit 固定 not_audited | cmd: cargo test trust_center --lib && cargo test --test trust_center_cli | ✅ | JSON 清单，非托管页面/外部审计 |
| eval 生成评测披露：suite/version/passed/total + 报告 SHA-256，可被 trust-center 读取 | cmd: cargo test --test eval_disclosure_cli | ✅ | 披露绑定本地报告；不证明评测真实/外部复现 |
| 单位经济：contribution/net/gross margin bps/每结果成本/break-even | cmd: cargo test unit_economics --lib && cargo test --test unit_economics_cli | ✅ | 调用方提供价格/成本；不内置定价 |
| Agent Card well-known 公开发现路由与 /v0/agent-card 同 payload | cmd: cargo test http_surface --lib | ✅ | 发现入口；非 A2A registry 发现/签名条目 |
| 安静时段持久化：onboard --quiet-hours 写入 preference，UserModel 读取并回显；autonomy-for 读取域档位+安静时段 | cmd: cargo test user_model --lib && cargo test --test onboard_cli && cargo test --test autonomy_cli | ✅ | 偏好视图；无窗口结束唤醒/调度 |
| Agent Card 信任锚：受信/吊销公钥库；未受信/已吊销/篡改拒绝 | cmd: cargo test card_trust --lib && cargo test --test card_trust_cli | ✅ | 本地信任锚，非远程密钥发现 |
| 团队交接：必须有 evidence、from≠to；仅 addressee 可接受；终态不可再转换 | cmd: cargo test handoff --lib && cargo test --test handoff_cli | ✅ | 本地交接；无实时共享上下文/IdP |
| 交接投影：open + to=recipient → inbox review，`handoff:<id>` 去重，不自动接受 | cmd: cargo test inbox --lib && cargo test --test handoff_inbox_cli | ✅ | 仅投影；通知/实时上下文未实现 |
| Tool search 按名称/前缀/描述/参数名确定性排序；run --tool-query 限制并记录模型可见工具 | cmd: cargo test tool_search --lib && cargo test --test tool_search_cli && cargo test --test tool_query_cli | ✅ | top 8 + always-on；词法而非语义；无 query 时全量 |
| 读权限关闭时终端执行工具不暴露给模型 | test: write_tools_are_hidden_without_write_permission | ✅ | 工具注册表按权限过滤 |
| 终端工具可在真实 PTY 会话中创建、执行并销毁 | cmd: cargo test --offline --test terminal_tools_live -- --ignored | ✅ | 需本机 skynet-term-sessiond 与 skynet-workshop-session；默认测试集跳过 |
| 1A 在写权限下用终端工具启动 copilot 并读取 GitHub 登录状态 | file: docs/evidence/terminal-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机验收；gh 已登录，未做交互式 device-code 登录 |
| 本地 CLI/TUI 默认允许执行，`--read-only` 关闭 | test: default_run_allows_execution_and_read_only_blocks_it | ✅ | CLI 层；使用 mock provider |
| TUI 默认开启执行，无需手动 `/permissions write on` | file: docs/evidence/terminal-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；tmux PTY；journal `allow_write=true` |
| 文件写入/编辑工具受写权限、根目录、符号链接和覆盖保护 | cmd: cargo test --offline --lib tools::files | ✅ | 原子替换；路径与符号链接有独立测试 |
| fetch_url 只读公网 HTTPS，拒绝私网/非文本/超大响应 | cmd: cargo test --offline --lib tools::web | ✅ | 纯函数 + 权限测试；实机抓取见 docs/evidence/web-2026-11-09 |
| 1A 可用 fetch_url 完成真实网页读取 | file: docs/evidence/web-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；example.com |
| 1A 可用 web_search 返回真实搜索结果 | file: docs/evidence/web-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；Bing RSS；结果可能本地化/限流 |
| MCP stdio client 可发现并调用工具 | cmd: cargo test --offline --lib tools::mcp | ✅ | fake server 契约测试；写工具权限门控 |
| 1A 可通过 `--mcp-config` 调用真实 MCP 工具 | file: docs/evidence/mcp-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；本地 fake echo server |
| 子 Agent 可隔离运行、回传结论并限制深度 | cmd: cargo test --offline --lib tools::subagent | ✅ | fake program；工作区越界与深度上限测试 |
| 1A 可 spawn 真实子 Agent | file: docs/evidence/subagent-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；两次独立子运行；父读子工作区被拒 |
| 定时任务可持久化、到期执行并更新状态 | test: schedule_store_and_daemon_once_run_a_job | ✅ | CLI e2e；mock provider；pid 锁与原子状态 |
| schedule 存储校验、锁和原子写入 | cmd: cargo test --offline --lib scheduler | ✅ | 到期/标记/删除/损坏状态测试 |
| schedule 支持文件 mtime trigger | test: schedule_trigger_file_runs_once | ✅ | CLI e2e；第二次未变化不重复运行 |
| cost 命令输出 token/成本与 p50/p95 延迟 | test: cost_reports_usage_and_latency_percentiles | ✅ | CLI e2e；按 provider/model 聚合 |
| 安装脚本可把二进制装入自定义 prefix 并运行 | test: installer_places_a_runnable_binary_in_a_prefix | ✅ | 临时 prefix；未覆盖系统级卸载 |
| digest 可汇总最近运行、计划任务、健康状态与告警计数且不调用模型 | test: digest_reports_recent_runs_and_schedules | ✅ | CLI e2e；JSON 输出；告警按 kind 计数 |
| 1A 可通过工具注册/评测/激活/回滚技能 | cmd: cargo test --offline --lib tools::skills | ✅ | 临时 skill journal；写工具权限门控 |
| 保留期可 dry-run/apply 删除过期 run journal 并写 receipt | test: retention_dry_run_and_apply | ✅ | CLI e2e；默认 dry-run |
| 委托 profile 可注册/列出/撤销并持久化 | cmd: cargo test --offline --lib tools::delegation | ✅ | 临时 store；写权限门控 |
| provider 告警日志可按 provider/model 汇总 | test: provider_alerts_summarizes_the_alert_log | ✅ | CLI e2e；真实 webhook 接收端未覆盖 |
| Surface goal/commitment 列表支持 limit/cursor | test: goal_and_commitment_lists_paginate_by_cursor | ✅ | 单元测试；skill 列表同代码路径 |
| browser_read 校验网络权限/HTTPS 并抽取 DOM | cmd: cargo test --offline --lib tools::browser | ✅ | fake Chrome；私网校验复用 fetch_url |
| 1A 可用无头 Chrome 渲染公网页面 | file: docs/evidence/browser-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；example.com |
| notify 可向 loopback/HTTPS webhook 发送通知 | test: notify_sends_a_loopback_webhook | ✅ | CLI e2e；真实 HTTPS 接收端未覆盖 |
| 记忆整合保留最新副本并软遗忘旧副本 | test: duplicates_keep_the_newest_and_apply_soft_forgets_the_rest | ✅ | 单元测试；dry-run 默认 |
| 上下文图缓存按输入指纹命中并可失效 | test: cache_hits_and_invalidates_on_fingerprint_change | ✅ | 文件缓存；Surface 接线测试 |
| TUI 输入框可用鼠标拖拽选择并反色显示 | test: ratatui_composer_centers_input_and_supports_mouse_selection | ✅ | 整个 composer 为命中区；另有 transcript 选择测试 |
| TUI 编辑在粘贴、空格、退格、方向键和鼠标混合序列下保持 UTF-8 光标安全 | test: editing_fuzz_keeps_the_cursor_on_a_character_boundary | ✅ | 2500 次确定性随机编辑；PTY 脚本 `scripts/pty_edit_stability.py` 另有 400 次真实输入 |
| TUI 运行卡显示中英双语阶段、第 N 步、请求/返回摘要、token、缓存和上下文进度 | test: tool_activity_shows_step_and_context_with_minimal_fence | ✅ | `第 N 步` 是实际模型请求序号；`⎿` 行显示请求消息数/上下文/工具数和返回类型；上下文条为 `─`，宽屏 20 格、窄屏 10 格 |
| 模型思考行显示请求/返回摘要，Ctrl+O 打开过程详情 | test: tui_run_trace_lists_requests_and_responses | ✅ | 请求消息/上下文/工具数；返回最终答案或工具调用；详情含逐事件过程记录 |
| 本地提示词库可解析 Felix / 卡帕西等作者 | test: prompt_library_facts_resolve_felix_and_karpathy | ✅ | Felix→Felix Rieseberg；卡帕西→Karpathy；命中条目注入会话上下文 |
| 追问请求注入上一轮上下文与短回复提示 | test: follow_up_request_gets_library_and_short_reply_context | ✅ | `known_facts` 含 Felix Rieseberg 与 `Short reply`；运行时以 `CONVERSATION CONTEXT` 系统消息注入 |
| 上下文预算按 token 计量，默认 1M，自动压缩阈值 786,432 | test: legacy_character_context_budgets_migrate_to_tokens | ✅ | `RunBudget`/SessionPreferences 迁移旧字符配置；provider usage 校准；token 估算测试 `token_estimates_handle_ascii_and_cjk` |
| 交互式 TUI 默认不限制步数、工具调用、模型尝试和运行时长 | test: zero_budgets_do_not_stop_a_long_run | ✅ | `0`=不限；17 次工具调用后仍完成；后台 run/daemon/子 Agent 仍有界；`zero_budgets_are_valid_and_mean_unlimited` 覆盖会话配置校验；单次模型 HTTP 调用 300 秒上限 |
| token 压缩在达到阈值时触发 | test: token_compaction_triggers_before_the_hard_limit | ✅ | 旧 `compact_messages_at` 字符接口保留兼容；journal 同时记录 token/字符 |
| Master 状态卡覆盖理解/规划/批准/分配/启动/Worker/收集/验收/汇报 | test: activity_taxonomy_is_bilingual_and_color_coded | ✅ | 持久卡片；下一步命令写在 footer |
| 工具调用和结果实时写入主会话工作日志 | test: work_entries_use_a_bilingual_label_in_the_transcript_log | ✅ | journal 增量驱动；状态与详情用 ` · ` 分隔；真实 PTY 证据 `docs/evidence/tui-2026-11-10/work-log.txt`；Bash 输出有长度上限 |
| 整个 TUI 大背景为暗绿色，为输入面板 80% 亮度且更暗 | test: terminal_backdrop_is_darker_green_than_the_input_panel | ✅ | 大背景 `#06100A`、输入面板 `#08140D`；欢迎页、主会话和命令面板共用大背景 |
| 欢迎页 `1` 保持亮绿，`Agent`/底部文字深绿，域名亮绿 | test: welcome_logo_keeps_one_green_and_paints_agent_deep_green | ✅ | 真实 PTY：`1`=`3fb950`，`Agent`/说明=`487748`，`SKYNET.CLUB`=`00ff00`；退出 banner 同步 |
| TUI 输入框/状态/模型/模式共用深绿围栏，token 指标嵌在顶部边框右侧 | test: composer_and_panel_text_share_one_fence | ✅ | 暗绿背景/边框/文字整体压暗 50%、白色输入；底部模型/模式；`T` 亮绿加粗；上下文只在顶部进度条 |
| 窄终端时 token 指标折入围栏内下一行，进度条减半，不再截断 | test: narrow_terminal_folds_metrics_inside_the_same_fence | ✅ | 72 列真实 PTY 证据 `docs/evidence/tui-2026-11-10/input-panel-folded.txt`；进度条 20→10 格，百分比不变 |
| 用户消息整行使用会话框背景 `#08140D`、白色文字 | test: user_message_and_answer_are_separated_by_an_empty_gap_row | ✅ | 真实 PTY 采样用户行 bg=08140d；回答前的空白行不继承该背景 |
| 右键粘贴仅在输入框内部触发 | test: right_click_paste_is_limited_to_the_composer | ✅ | 输入框矩形命中测试；真实 PTY：框外右键无提示，框内右键出现“已从剪贴板粘贴”；Ctrl+V 不受限 |
| 工作日志中的本地路径与 URL 为亮绿色 | test: work_log_colors_links_bright_green | ✅ | 绝对/相对路径与常见扩展名；URL 仅 http(s)；统一使用 WorkLink 亮绿色 |
| 工作日志动作名称和 `•` 为亮绿，详情与 `⎿` 结果为深绿且不加粗 | test: work_log_uses_bright_green_labels_and_deep_green_details | ✅ | 80x20 TestBackend：动作名如“模型思考 Thinking”为 LightGreen；详情/结果 `#487748`；均无 BOLD |
| 最终答案使用“结论 Conclusion”深绿边框、亮绿加粗标题、白色正文 | test: final_answer_is_wrapped_in_a_conclusion_fence | ✅ | direct/structured 共用；围栏内上下各留一空行；真实 PTY 证据 `docs/evidence/tui-2026-11-10/conclusion-fence.txt` |
| 结论正文支持 Markdown 标题、列表、任务框、分隔线和链接高亮 | test: conclusion_markdown_renders_headings_bullets_and_bold_labels | ✅ | `##` 粗体标题；`- ` 项目符号；`1.` 有序行；`- [x]`→☑/☐；`---`→分隔线；`**重点**` 加粗标签；URL/路径使用链接色；正文不残留 `**` |
| 结论正文支持列宽对齐的 Markdown 表格 | test: conclusion_markdown_renders_an_aligned_table | ✅ | 表头、分隔行和数据行等宽；超宽单元格截断并补空格；窄终端按行折行 |
| 系统提示要求多维度答案使用短段落/列表/对比表，避免一整段长文 | test: system_prompt_carries_the_occam_style_laws | ✅ | 新增 `Format for scanning` 规则；Markdown 允许且优先用于提升可读性 |
| 结构化推理区块全部深绿，只有结论答案为白色 | test: structured_sections_are_deep_green_while_the_answer_stays_white | ✅ | 主张/依据/假设/约束/未知/冲突/翻转条件的标题、标签、项目符号与正文均为 `#487748`；围栏内答案为白色 |
| `/` 命令抽屉的指令名称和说明均为亮绿色 | test: command_drawer_uses_bright_green_entries | ✅ | 名称加粗、说明不加粗；窗体背景 `#0A1F12`，选中行 `#143D24` |
| 所有 TUI 弹窗（详情/命令面板/抽屉/模式菜单）使用绿色系背景 | test: interaction_overlays_use_green_surfaces | ✅ | 背景 `#0A1F12`、选中 `#143D24` 均为绿系；详情边框亮绿；真实 PTY 采样确认 |
| `/` 命令抽屉可搜索 48 条提示词补充 | test: prompt_supplements_are_featured_and_searchable | ✅ | 按名称/分类/触发/用途搜索；中文空查询含 3 条精选；输入 `//` 浏览全部 48 条；English 空查询列出 14 条有英文原文的卡片；真实 PTY `/验收` 命中 |
| 选中提示词插入正文并保留变量，不自动执行 | test: prompt_supplement_completion_fills_the_composer | ✅ | 输入框填入模板，`{{变量}}` 留给用户；真实 PTY 确认 |
| 已写任务后输入 `//` 打开原文库并追加到任务 | test: prompt_drawer_opens_after_a_task_and_merges_the_source_text | ✅ | 原文追加到原任务，保留完整正文；不执行 |
| 任务内 `//关键词` 搜索，Esc 恢复原任务 | test: prompt_drawer_searches_inside_a_task_and_escape_restores_it | ✅ | 抽屉只显示提示词；Esc 去掉 `//` 查询 |
| 空输入框输入 `//` 打开完整原文库并按分类分组 | test: double_slash_opens_the_whole_source_library | ✅ | 飞书表 56 行归为 32 组；按“作者+主题”分组、共性分类打标签；`↑`/`↓` 选择、`Enter` 插入、`Esc` 关闭 |
| 原文库插入完整原文，不截断、不改写 | test: double_slash_opens_the_whole_source_library | ✅ | 中文插入完整中文直译；English 插入完整英文原文；只插入正文，作者/主题/分类/来源链接只留在抽屉行；测试断言输入包含整段 `translation`/`original` 且不含标题与参考行 |
| 同一段原文重复插入不会重复堆叠 | test: inserting_the_same_source_twice_does_not_duplicate_it | ✅ | 输入框已包含正文时跳过并提示，保留原任务意图 |
| 原文库按作者+主题归组并保留源表记录 ID | test: prompt_drawer_without_a_task_lists_the_whole_source_library_by_category | ✅ | 32 组；Think Before Coding 含 6 条片段；生成脚本保留 `record_ids` 与来源链接 |
| 原文库每组与飞书表逐行原文完全一致 | test: source_groups_keep_every_table_row_complete | ✅ | 重新解析快照 ndjson，逐组比对 `original`/`translation` 拼接结果与行数，防止导入截断或改写 |
| 原文库在宽终端预览选中条目的完整正文 | test: source_drawer_previews_the_complete_body_before_insert | ✅ | 右侧预览作者/主题/分类/字数与完整正文；窄终端仍可直接插入 |
| 提示词语言可切中文/English；原文库按语言插入完整中文直译或英文原文 | test: english_source_mode_inserts_the_complete_english_original | ✅ | `/config set language zh\|en`；`//` 原文库 32 组两种语言都可用；`/` 英文精选仍只列有逐字原文的 14 条；不用中文回退 |
| 技能包导出/导入受 SHA-256 门禁，篡改或哈希不符即拒绝 | test: import_rejects_a_tampered_bundle | ✅ | 导入先预检全部 manifest，坏单元不会半导入；CLI e2e 覆盖 |
| 技能包可用 Ed25519 签名，导入需可信公钥并拒绝篡改 | test: signed_bundles_require_the_trusted_public_key_and_reject_tampering | ✅ | CLI e2e `signed_skill_bundles_verify_against_a_trusted_public_key`；无私钥打印、私钥文件强制 0600；无远程市场/撤销列表 |
| Routine 只使用已激活的精确技能版本，未激活或版本不符即拒绝 | test: routine_requires_an_active_exact_version_and_composes_the_task | ✅ | 任务中显式注入技能正文；不授予额外权限 |
| daemon 重启后会补跑停机期间已到期的持久化任务 | test: daemon_catches_up_an_overdue_job_after_restart_and_health_reports_it | ✅ | CLI e2e；模拟 next_run 已过；未验证长周期真实每日任务 |
| 计划任务失败/超时时 daemon 写告警日志并可发送 webhook | test: daemon_alerts_when_a_scheduled_run_fails | ✅ | loopback 接收端；无升级阶梯、跨重启去重与外部送达验证 |
| schedule health 区分 ok/overdue/failing | test: health_reports_ok_overdue_and_failing_jobs | ✅ | 只读快照；自动告警/外部接收端未实现 |
| cost 可按任务聚合运行数、token、成本，并统计每个工具调用/失败次数 | test: usage_ledger_aggregates_every_model_response | ✅ | 仅从 journal 派生；工具级 token 归因未实现 |
| dashboard 生成自包含 HTML 看板并转义所有动态内容 | test: renders_sections_and_escapes_dynamic_text | ✅ | 另有 CLI e2e；静态报告，不是实时服务；含任务正文，文件权限 0600 |
| audit 导出可按 seq 分页并按需返回记录 | test: audit_store_reads_task_graph_and_memory_receipts | ✅ | CLI e2e；默认仍只返回 receipt；记录含 journal 内容，需 audit:read 权限 |
| notify 可输出 Slack/Discord 的 webhook payload 形状 | test: notify_sends_slack_and_discord_payload_shapes | ✅ | loopback 接收端；未验证真实 Slack/Discord workspace，邮件/日历未实现 |
| read_pdf 提取本地 PDF 文本，并受读根、符号链接、控制字符和字符上限约束 | cmd: cargo test --offline --lib tools::documents | ✅ | fake pdftotext 单测；真实提取另见下一条 |
| 1A 可用真实 pdftotext 从生成的 PDF 提取文本 | cmd: cargo test --offline --test pdf_live -- --ignored | ✅ | 需 poppler-utils + ghostscript；仅 Linux 后端；无 OCR/图片理解 |
| 全量门禁通过（fmt、clippy、测试、评测） | cmd: ./scripts/verify.sh | ⚠️ | 耗时较长，等价于 fmt + clippy + test + eval |

## 发布与复现

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| v0.94.0 源码基线可验证 | file: docs/baseline-2026-11-04.md | ✅ | |
| 发布二进制摘要与文档记录一致 | 无 | ❌ | 文档记 864ffaf0，重建为 dd4faf8e，见基线说明 |
| 跨机器可复现构建 | 无 | ❌ | 只证明了本机重建 |
| Windows 真机运行 | 无 | ❌ | 仅交叉编译 |

## 质量声明

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| 试点语料覆盖 7 个任务族，且每例可在 2 问内到达动作 | cmd: python3 scripts/reframing/analyze_hypothesis_space.py fixtures/reframing/pilot-v0.json | ✅ | 设计层校验，不是模型行为 |
| 在 11 个构造的 means 案例上，10 个得到 2–3 路问题 | file: docs/evidence/reframing-pilot-v0/summary-v2.json | ⚠️ | 调参集 10/11；held-out 召回 21/21；但 precision 未达标 |
| 产品能自行生成重构提议 | file: docs/evidence-reframe-live-v0.md | ⚠️ | 一次真实调用；提示词与研究脚本同源，哈希已锁 |
| 语义判断可被两位独立标注者复现 | 无 | ❌ | 两轮由同一人填写，κ 无效；需第二位独立标注者 |
| 替代方案可接受率 | 无 | ❌ | 同上：只有一位标注者的判断，不能代表用户 |
| 真实模型的决策质量 | 无 | ❌ | 无 held-out 结果 |
| 回答不复述请求、不含开场脚手架 | file: docs/evidence-answer-style-v2set.md | ⚠️ | 两套独立 14 任务均达标：散文 p50 292/222，句子 4.0/5.0，复述 2/14、2/14，脚手架 0 |
| 五条风格法已写入系统提示词 | test: system_prompt_carries_the_occam_style_laws | ✅ | 只证明条款存在，不证明模型遵守 |
| 输出风格基线已建立 | cmd: python3 scripts/answer_style/baseline.py | ✅ | 19 条可测答案，全部来自加规则之前的提示词 |
| 并列方案不超过 3 个且给出默认推荐 | 无 | ❌ | 契约已写，未校验 |
| 未决定时给出可执行的方案菜单 | test: no_decision_prints_a_menu_and_writes_nothing | ✅ | CLI 层；TUI 未接；方案菜单按设计无默认项 |
| /reframe 可被 Ctrl+C 取消且不残留提议 | cmd: python3 scripts/pty_reframe_cancel.py | ✅ | PTY，本地黑洞端点；取消后会话仍可用 |
| 重构失败时给出可读错误且不泄露端点 | cmd: python3 scripts/pty_reframe_failure.py | ✅ | PTY，连本地关闭端口；失败后会话仍可用 |
| TUI 内可完成需求重构与决定 | cmd: python3 scripts/pty_reframe.py | ✅ | PTY 真实输入，断言提议面板与决定落库 |
| 每轮结束提供带默认项的动作菜单 | cmd: python3 scripts/pty_next_actions.py | ✅ | 真实 PTY 驱动 TUI，断言面板出现且默认项有标记 |
| 回答语言与请求一致 | file: docs/evidence-answer-style-v3.md | ⚠️ | 全新 14 个任务 0 条语言错配；关掉语言条款的对照曾出现 1 条 |
| 菜单项只来自已注册命令 | test: every_entry_carries_a_real_command_name | ✅ | 只证明在注册表内；`implemented` 标志恒为 true，见下方缺陷 |
| 真实语料中"手段类请求"的占比 | 无 | ❌ | 75 条 journal 中真实请求约 10 条，样本不足 |
| 需求重构带来的成本下降 | 无 | ❌ | 未做对照实验 |

## 覆盖率

本表逐条覆盖的条目数，以 `scripts/verify-claims.sh` 的输出为准——不在文档里写死，
避免出现第二个会漂移的数字。README 的 `## Status` 能力清单尚未逐条映射。

**未映射不等于不成立，也不等于成立——只等于没人核对过。**
