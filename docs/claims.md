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
| 替代方案必须列出新增代价 | cmd: cargo test --test reframe_costs_cli | ⚠️ | `added_costs` 字段与 `--require-costs` 校验已实现；默认提示词/模型输出未强制列出；未知/空成本失败关闭 |
| 决定权在用户，只读回答不得产生动作回执 | test: read_only_answer_rejects_action_receipts | ✅ | |
| 被拒绝后不再重复提议 | cmd: cargo test --test reframe_repeat_cli | ✅ | 扫描最近 500 个 journal 的同一 stated_request rejected 决定；--force 覆盖；仅 CLI reframe 路径，非全局对话记忆 |
| 可查询历史决策收据 | cmd: cargo test --test receipt_index_cli && cargo test tui_commands --lib | ✅ | CLI 跨 journal 索引最近 200 条；TUI `/receipt` 读当前会话最近决策；依赖本地 journal，不跨机器 |

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
| 流式 provider：`run --stream` 实时输出内容增量，tool_call 分片按 index 拼装，SSE 行跨包重组，取消返回 Cancelled，未实现流式的 provider 回退 complete，重试不流式 | cmd: cargo test stream --lib && cargo test --test stream_cli && cargo test --test openai_adapter -- --ignored | ✅ | 仅 CLI stderr 可见，TUI 未接；多行 `data:` 事件不支持；增量是展示层，不是证据；provider 不上报 usage 时预算会少计 |
| 连续只读工具并发执行，写/终端/技能/子 Agent/MCP 保持顺序；ToolCalled/ToolObserved 均按模型顺序写 journal | cmd: cargo test parallel_safe_tool_calls --lib && cargo test write_tools_do_not_overlap --lib && cargo test tool_groups --lib | ✅ | 并发仅在同一模型批次内；中断的并发组不可续跑；无全局并发上限 |
| journal 增量读取只解析游标后的完整行，未换行尾部保留，文件缩小失败关闭；4.7 MB（约 1.2M token）全量解析 32 ms、replay 0.32 ms、增量 p95 9.4 µs（M16 ≤50 ms） | cmd: cargo test incremental_read --lib && cargo test journal_full_parse_at_one_million_tokens --lib -- --ignored --nocapture && cargo test incremental_read_benchmark --lib -- --ignored --nocapture | ✅ | 时间为本机调试构建；TUI/daemon 消费方尚未切换；未读尾部仍整体读入内存；journal 为合成事件 |
| checkpoint 差分：首个全量、后续只写新增消息；resume/replay 顺序合并、base 不匹配失败关闭、audit 标记孤立 delta；100 步 checkpoint 事件 115 KB（全量估算 2.99 MB） | cmd: cargo test checkpoint_delta --lib && cargo test delta_checkpoints_keep_the_journal_linear --lib -- --ignored --nocapture | ✅ | 仅 checkpoint 事件；旧二进制不能读含 delta 的新 journal |
| 模型请求差分：首个全量、后续 `ModelRequestDelta` 只写未变前缀之后的消息、工具变化时才带 tools；链式重建、orphan/prefix 越界由 audit 拒绝；100 步 echo run 模型请求事件 74.9 KB（全量估算 3.03 MB，2.5%） | cmd: cargo test model_request_deltas_reconstruct_the_full_request --lib && cargo test audit_flags_an_orphan_model_request_delta --lib && cargo test audit_flags_a_model_request_delta_prefix_overflow --lib && cargo test delta_checkpoints_keep_the_journal_linear --lib -- --ignored --nocapture | ✅ | 仅同机 echo run 的 journal 字节；不证明 provider wire/CPU 节省；旧二进制不能读新 variant；`replay_events` 不解析请求内容 |
| TUI transcript 投影缓存：内容 revision/宽度不变时复用；新增 entry 或 resize 重建；200 条长 transcript release 帧 p95 1.26 ms（M15 ≤8 ms） | cmd: cargo test transcript_projection --lib && cargo test --release long_transcript_frame_budget --lib -- --ignored --nocapture | ✅ | TestBackend 测量，不含真实终端 I/O；debug p95 11.1 ms；缓存按 revision 全量重建，非增量 diff |
| TUI 工作日志：模型请求每步只一行，显示 provider/model 与“新增/共 M 条（系统/用户/助手/工具）”，不再显示上下文 token/可用工具数；下一事件不重复追加；工具结果按 call_id 紧贴调用；常见网络/超时/HTTP 失败中文化并保留 URL；宽表堆叠；结束原因中文化 | cmd: cargo test --offline --bin first-principles-agent append_work_events_does_not_duplicate_a_model_request && cargo test --offline --lib parallel_tool_results_are_inserted_under_their_calls && cargo test --offline --lib a_table_that_would_truncate_is_stacked_instead && cargo test --offline --bin first-principles-agent tool_results_pair_the_tool_name_and_humanize_failures | ✅ | TestBackend；真实终端宽度/主题未逐一截图；错误文案是启发式映射；prompt 不能保证模型少追问 |
| watchdog 升级阶梯按心跳年龄输出 ok/warn/pause/repair；干预日志只在动作变化时追加、恢复记一次；默认只建议；`--execute pause/resume` 校验 cmdline 后发 SIGSTOP/SIGCONT，拒绝自身/缺失/不匹配 | cmd: cargo test watchdog --lib && cargo test --test watchdog_cli | ✅ | 不 kill/不重启/不自动恢复；Unix only；cmdline 检查与信号之间有 TOCTOU；日志无轮转 |
| watchdog 自动遏制：`--watch --execute pause` 按 `--interval-ms` 持续评估，对每个 Pause/Repair pid 只发一次校验过的 SIGSTOP；Pause→Repair 不重复；心跳恢复或 pid 变化后才允许再次发送；支持 `--ticks`；拒绝自动 resume | cmd: cargo test --test watchdog_cli watchdog_watch | ✅ | 仅 Unix；只自动暂停，不自动恢复/重启/清锁；信号失败即退出；自动恢复需独立健康信号（未实现） |
| Clarify 门控：2–4 个唯一选项、至多一个推荐项、自动 Other、推荐项第一、默认每轮 1 题、总预算/重复过滤、blocking 批量 ≤5；答案状态 answered/skipped/unanswered、结果 submitted/cancelled/timed_out/undelivered；未回答不使默认答案失效；`ClarifyPanel` 批量状态机（逐题锁定/Other 文本/一次确认/取消/300s 超时）；TUI `/clarify` 全屏面板（↑↓ 选项、←→/Tab 切题、Enter 选择/提交、S 跳过、Esc 取消、Other 内联编辑、500ms 超时轮询）；交互式 TUI 注册 `clarify` 工具，模型可主动发起，结果作为工具观察返回 | cmd: cargo test clarify --lib && cargo test --test clarify_cli && cargo test tui_clarify --lib && cargo test clarify_panel --lib | ✅ | 行/headless 不注册工具；持久化问题历史未实现；鼠标/IME 未覆盖 |
| 错误恢复建议：provider 鉴权/限流/超时/网络/非法响应、journal、取消、resume、路由等失败在 CLI 输出一条 `hint:`，不改变错误/重试/权限/退出码；TUI 失败后追加 `提示：...` | cmd: cargo test runtime_errors_carry_actionable_hints --lib && cargo test --test cli_e2e provider_network_failure_prints_a_recovery_hint && cargo test --test cli_e2e tui_line_mode_prints_a_recovery_hint | ✅ | 建议是静态文案，不保证一次修复；TUI 交互模式与行模式共用同一 hint |
| 架构边界 RFC：session/harness/sandbox 三接口与 main.rs/tui.rs 拆分计划已记录 | file: docs/rfc/0001-module-boundaries.md | ⚠️ | 本地架构评审；拆分实现未开始 |
| 本地门禁指标：journal 投影 runs/completed/interrupted、恢复率、结构回放率、压缩回放率、工具成功率、审计完整率、拒绝观测、token/成本、run 延迟；零分母为 null，坏 journal 失败关闭 | cmd: cargo test journal_metrics --lib && cargo test metrics --lib && cargo test --test metrics_cli | ✅ | 结构回放非模型重跑；M6 拒绝观测只是代理；M7 TTFT/M14/M19–M23 未计算 |
| 阶段耗时投影：metrics 从 journal 时间戳输出 route/context/model/tool p50/p95；四段互不重叠；重试只计 model | cmd: cargo test journal_metrics_project_stage_latencies_from_timestamps --lib && cargo test --test metrics_cli | ✅ | 毫秒精度；journal 写入/TUI 渲染未覆盖；结构投影非 CPU 时间 |
| 指标门禁：`metrics --require-*-bps` 按阈值失败关闭；缺样本不当作 0；`0` 禁用；`scripts/gate-check.sh DIR` 默认 M4≥95%、M5/M13/M17=100%；`release-check.sh --metrics-dir` 接入 | cmd: cargo test metrics --lib && cargo test --test metrics_cli | ✅ | 门禁只反映所给 journal 目录；M6 需单独安全套件；无托管 CI 服务；`gate-check.sh DIR` 需真实 journal 目录，由证据示例覆盖 |
| Golden set v1：40 条固定 prompt（五类各 8 条）；`golden-check` 校验 30–50 条、唯一 id/prompt、安全字符、类别覆盖 | cmd: cargo test golden --lib && cargo test --test golden_check_cli | ✅ | 只校验清单；不做盲评/不跑模型；40 条不代表统计充分 |
| 真实任务套件 v1：8 个真实任务；`EvalCase.allow_write` 逐例显式开启写权限；`eval --validate-only` 校验结构/元数据/唯一 id，不跑模型；无 `--config` 拒绝运行 model_quality | cmd: cargo test shipped_real_task_suite_is_structurally_valid --lib && cargo test --test eval_validate_cli | ✅ | 已在 DeepSeek 上运行两次（11/16）；rubric 未人工校准；M1/M2/M3 仍未测量 |
| 真实任务基线：DeepSeek `deepseek-flash`，suite v1.1 两次运行 16/16（100%），rubric 36/36；v1.0 校准后三次 17/24（70.83%），1 次硬门失败 | file: docs/evidence/real-tasks-benchmark-2026-11-12.md | ⚠️ | 单 provider/单模型；8 任务×2 次，样本小；未人工评审/校准；不是 M1 8h |
| 旧 journal 兼容：Evidence 时间戳与 ReframeDecided 使用 u64，含时间戳的 tagged event 可读；JSON 数字格式不变 | test: tagged_events_with_evidence_timestamps_round_trip | ✅ | 修复 serde tagged-enum 的 u128 缓冲限制；其它应用层 u128 字段若进入 tagged payload 仍需转换 |
| TUI 纯文本函数已拆到 `tui_text`（sanitize/fit/wrap/truncate/split），`tui.rs` 重新导出；行为不变 | cmd: cargo test tui_text --lib && ./scripts/tui-interaction-check.sh | ✅ | 仅第一批拆分；transcript/input/command panel/state 与 main.rs 未拆 |
| 答案渲染已拆到 `answer_render`（完整/分层/标签），`main.rs` 导入；输出不变 | cmd: cargo test answer_render --lib && cargo test --test cli_e2e default_run_output_is_concise_and_details_expands_it | ✅ | 仅渲染函数；main.rs 命令组/路由轨迹未拆 |
| 回答质量看板：从 JSONL 样本计算 M19–M23；M20 平局排除、零分母 null、非法行报行号、追问采纳必须有追问 | cmd: cargo test answer_quality --lib && cargo test --test answer_quality_cli | ✅ | 只计算调用方标签；不执行盲评/不采集 golden set；标签质量不受控 |
| core-v3 八例契约套件 8/8 且两次运行判定一致；报告 digest 含 duration，不逐字节比较 | cmd: cargo test --test eval_repeatability_cli && cargo run --offline --quiet -- eval --suite evals/core.json | ✅ | 契约夹具，不是真实任务/模型质量；历史 6/6 文档不回写 |
| 0.97 指标复核：硬门/M1–M23 现状、证据与阻塞项已记录；未测项明确标为未测而非 0 | file: docs/status-v0.97.md | ⚠️ | 本地复核；独立代码级评审 0 P0/5 类 P1 已修、4 项 P1 残余，但无第三方 A-Gate/W-Gate/认证；M19–M23 为 AI/批量标签，不是人工盲评 |
| 0.97 回滚与已知限制：二进制/journal/config 回滚、kill switch、事故流程与限制已记录；旧二进制不能读含 checkpoint_delta/model_request_delta 的新 journal | file: docs/evidence/rollback-drill-2026-11-12.md | ⚠️ | 已完成 0.96.1↔0.97 journal 读取演练；config/session/kill switch 升级回滚与生产演练未做 |
| 基准运行器：重复执行 eval suite，输出完成率/crash-free/结构回放并经 benchmark 门禁；失败非零，进程崩溃不算样本；新增任务级 M1（tasks_total/tasks_passed/task_completion_bps）与 completed-run M5（completed_cases/completed_replay_ok/completed_replay_bps），M5 只统计 `Completed` 且要求 audit+replay；`--min-task-completion-bps`/`--min-completed-replay-bps` 独立设门 | cmd: cargo test --test benchmark_run_cli && cargo test --offline --lib eval:: | ✅ | 契约夹具+确定性 provider；真实 8h/M1 仍需 model_quality suite；旧 8h JSON 无任务级字段；回放为结构 replay，不是模型重跑 |
| answer repair 无工具：repair 请求 tools 为空，high-impact assumption 必须降级，evidence.source 要求精确路径 | cmd: cargo test --offline --lib invalid_final_answer_is_repaired_once_and_revalidated && cargo test --offline --lib unparseable_provider_text_is_preserved_and_repaired | ✅ | 修复仍可能返回无效 JSON 并硬错误；prompt 不能保证真实模型遵守；未用真实 provider 重跑 |
| 记忆召回解释：按 bot scope 返回命中分数/来源/taint/置信度/有效期；空 query 失败关闭；输出可能含解密内容 | cmd: cargo test --test memory_recall_cli | ✅ | 子串/词项打分非语义检索；无冲突分组；解密内容需防日志泄露 |
| Trust Center 审计报告：调用方提供审计机构/范围/状态/起止/P0/P1/摘要；JSON/Markdown/转义 HTML；校验失败关闭；固定 audit.status 不变 | cmd: cargo test trust_center --lib && cargo test --test trust_center_cli | ✅ | 调用方提供、未独立验证；不改变 not_audited；P0/P1 不自证 |
| Cascade 基准：多场景遏制模拟，汇总最大深度/影响面/最小遏制率，任一失败非零；非法行报行号 | cmd: cargo test cascade --lib && cargo test --test cascade_bench_cli | ✅ | 确定性图模拟；非实时故障注入/24h 多 Agent 验收 |
| Python 只读 SDK：封装 health/agent-card/trust-center/surface；env/参数 token；4 MiB 上限；HTTP/JSON 错误抛 ClientError | cmd: cd sdk/python && python3 -m unittest discover -s . -p 'test_*.py' | ✅ | 无任务运行/流式/重试/async；不在 verify.sh 中；fake HTTP server 测试 |
| 看板门禁指标：dashboard 增加 crash-free/recovery/replay/tool/audit/denied/TTFT 卡片；静态转义 HTML | cmd: cargo test dashboard --lib | ✅ | 读 journal 两次；M19–M23 不在；M1–M3 仍需 8h benchmark；TTFT 已有 3 个真实样本 |
| 首 token 计时：`--stream` 首次尝试记录 provider/model/elapsed_ms；metrics 输出 TTFT p50/p95；非流式无样本 | cmd: cargo test --offline stream_sink_receives_deltas_before_the_final_answer -- --quiet && cargo test --offline journal_metrics_count_completion_tools_and_audit -- --quiet | ✅ | 真实样本 n=20（DeepSeek `deepseek-flash`，10 短 + 10 长，p50 1.50 s / p95 2.77 s）；单 provider、tools 关闭；每次重试单独计数；非统计充分 |
| M14/M17 本地证据：`crash_free_bps` 为有终止 stop 的 run 比例；强制压缩 run 可 replay/audit 且 compacted_replay_bps=100% | cmd: cargo test compacted_run_replays_and_audits --lib && cargo test journal_metrics --lib | ✅ | M14 是代理；M17 为确定性压缩夹具，非真实 1M 上下文 |
| 威胁模型：P0/P1 清单、信任边界、缓解与残余缺口已记录 | file: docs/threat-model.md | ⚠️ | 自评，无外部审计；CUA 执行器/运行 journal 哈希链/同 UID 隔离等缺口仍在 |
| 答案分层：默认只显示状态/结论/信任摘要；conditional、needs_evidence 或高影响未知时内联 ≤3 条依据与全部翻转条件；`--details` 恢复完整字段与路由轨迹 | cmd: cargo test --test cli_e2e default_run_output_is_concise_and_details_expands_it | ✅ | 高风险判定是启发式；低风险答案默认隐藏证据（摘要行给出计数）；TUI 已有 `/details` 与 `/why` |
| 崩溃注入恢复：从首个 checkpoint 续跑完成且不重放已完成副作用；ToolCalled 未观测或无 checkpoint 的 journal 失败关闭；可配周期/时长 soak | cmd: cargo test --test recovery_soak -- --ignored --nocapture | ✅ | 确定性本地 provider + 单写工具；不代表真实外部副作用/多 Agent 竞争；2h 时长门需单独运行 |
| 写路径重叠的任务被串行化 | test: overlapping_write_roots_are_serialized | ✅ | |
| 平台分支只允许出现在展示适配器 | cmd: ! grep -rnF 'cfg(target_os' src --exclude=tui_terminal.rs | ✅ | 本日修正：原门禁依赖缺失的 rg，被静默跳过 |
| 技能可从捕获、评测、激活回滚到上一版本，失败候选不替换现役版本 | test: skill_lifecycle_capture_to_rollback_is_replayable | ✅ | 确定性集成场景；基于磁盘 journal、重启回放与审计收据；不代表真实示范语料 |
| skill capture 只接受 Completed run，提取成功工具步骤并生成可扫描的 SKILL.md 草稿；密钥/凭据（含嵌套对象/数组、连字符键、转义引号）脱敏、对象键（含 secret 键显示）与路径/UUID/邮箱泛化、重复 frontmatter/多行元数据拒绝、read-time 64 MiB 上限；不自动注册/激活 | cmd: cargo test skill_capture --lib && cargo test --test skill_capture_cli | ✅ | 启发式脱敏，不是完整 DLP；只捕获成功工具步骤；草稿需人工评测/激活 |
| skill CLI 注册→静态 held-out 评测→仅通过后激活；失败评测非零且不改现役版本 | cmd: cargo test --test skill_register_eval_cli && cargo test skill --lib | ✅ | 标记匹配不是质量保证；suite 由操作者提供；canary 另算 |
| skill canary CLI：样本 JSONL；promote 激活候选/rollback 恢复 baseline/pending 不变；行号错误；100k/16 MiB 上限；`run --canary-sample` 追写 success/latency（无任务文本） | cmd: cargo test skill_canary --lib && cargo test --test skill_canary_cli && cargo test --test canary_sample_cli | ✅ | 样本由调用方提供；不持久化 canary 决策；success 仅 Completed |
| UserModel 2.0 可从记忆重建；自治/声音/同意默认安全，遗忘记录即失效 | cmd: cargo test user_model --lib && cargo test --test user_model_cli | ✅ | 自治与同意是偏好，不是授权；Authority/Policy/Approval 仍决定执行 |
| 平台能力状态显式；`untested` 不宣称支持 | cmd: cargo test platform --lib && cargo test --test platform_cli | ✅ | Linux 状态图；macOS/Windows/other 全部 `untested`；不是 P-Gate 通过 |
| CI 矩阵：Linux 完整 verify.sh；Windows/macOS all-targets check + test compile；YAML 结构合法 | file: .github/workflows/ci.yml | ⚠️ | 工作流未在 GitHub 实际运行；Windows/macOS 仅编译级，不是 P-Gate |
| 独立验证器拒绝自验证、重复/缺失结果和无证据通过；public pass + hidden fail 记为 hacked | cmd: cargo test verification --lib && cargo test --test hack_verifiable | ✅ | 确定性契约夹具；不代表真实模型质量；隐藏测试需与公开测试分开维护 |
| 可逆副作用无补偿时失败关闭；不可逆/失败需人工；away digest 分区可见 | cmd: cargo test repair --lib && cargo test --test away_digest_cli | ✅ | 只规划补偿，不执行；执行仍需 Authority/Policy/Approval |
| 价值周报只报可复现结果与每完成结果成本，不编造“省时”；包含 open inbox 计数 | cmd: cargo test value_report --lib && cargo test --test value_report_cli | ✅ | mock journal；真实用户价值需后续测量基线；inbox 与 alerts 未去重 |
| 三种伙伴声音进入系统提示；无效模式失败关闭；不改变证据规则 | cmd: cargo test voice --lib && cargo test --test voice_mode_cli | ✅ | 契约级；盲评质量未做 |
| 伙伴声音 v1：按认知模式附加 decision/research/execution 形态；TUI `/why` 展开结论/证据/假设/未知/翻转/收据 | cmd: cargo test voice --lib && cargo test --test voice_mode_cli && cargo test --bin first-principles-agent why_tests | ✅ | 形态由 CognitiveMode 映射；`/why` 只读最近回答；盲评 M20 未做 |
| Inbox 快照可重放；终端状态不可迁移；重复 ID/空字段/无效 snooze 失败关闭；CLI/TUI 可操作；list kind/source 过滤；sync 去重且默认 dry-run | cmd: cargo test inbox --lib && cargo test --test inbox_cli && cargo test --test inbox_sync_cli | ✅ | 桌面/聊天渠道投递未接；真实用户未验证 |
| 自治档位策略：observe/suggest 不行动；act 仅低风险可逆；auto 低/中风险可逆；高风险/不可逆需审批 | cmd: cargo test autonomy --lib && cargo test --test autonomy_cli | ✅ | 偏好不是授权；Authority/Policy/ExecutionGate 仍决定执行 |
| Onboarding 写入 memory 并可经 user-model 回读；非法值失败关闭；可 seed goal/commitment；`--interactive` 读 stdin；`--check` 只读检查路径/provider 环境且不创建文件 | cmd: cargo test --test onboard_cli && cargo test --test cli_e2e onboard_check_reports_paths_and_provider_without_writing | ✅ | 脚本化访谈；自由追问/文档/日历导入未做；memory 与 goal 写入非原子 |
| TUI `/inbox` 列出并可 resolve/snooze/reopen；解析与应用契约与 CLI 一致 | cmd: cargo test inbox --lib && cargo test tui_commands --lib | ✅ | 只操作 inbox store；不绕过 Authority/Policy/Approval |
| Inbox reopen：open/snoozed → open（取消 snooze），终态项拒绝 | cmd: cargo test --test inbox_cli | ✅ | 不重开终态；不改其他字段 |
| Agent Card 列出能力状态与限制（含 verification store/team handoff/inbox notify 多目标/inbox escalation/repair runbook+mark/org audit/MCP HTTP/chat bridge/daemon maintenance/trust center 事故披露/skill lifecycle/skill capture/voice forms）；unsupported/untested 不标为 verified；HTTP 公开只读提供 | cmd: cargo test agent_card --lib && cargo test --test agent_card_cli && cargo test http_surface --lib | ✅ | 签名可选；非 A2A registry-signed 卡 |
| skill scan 预检 frontmatter/控制字符/密钥/指令覆盖；不注册不执行 | cmd: cargo test skill --lib && cargo test --test skill_scan_cli | ✅ | 启发式扫描，不是 malware sandbox；签名/权限另算 |
| skill verify 与 import 共用 verify_bundle；校验 SHA/schema/签名/重复/manifest/扫描，不导入 | cmd: cargo test skill_bundle --lib && cargo test --test skill_verify_cli | ✅ | 证明完整性，不证明运行安全 |
| CUA 策略：untrusted high-risk deny；untrusted require_approval；planner 不看 untrusted；未规划 branch deny；`CuaPlan` 未冻结不授权、冻结后拒绝 replan；红队夹具覆盖屏幕注入改收款人/分支劫持 | cmd: cargo test cua --lib && cargo test --test cua_check_cli | ✅ | 仅策略，无浏览器执行/截图 |
| Cascade 模拟计算 affected/contained/propagated、containment_bps/depth/blast radius 并按策略门禁 | cmd: cargo test cascade --lib && cargo test --test cascade_check_cli | ✅ | 确定性模拟，不是真实多 Agent 运行 |
| Budget 硬上限：超限失败、80% 警告；每验证结果成本；超限非零退出；run 运行时累计并停止 | cmd: cargo test budget --lib && cargo test --test budget_check_cli && cargo test cost_budget_stops --lib | ✅ | 运行时按 provider-reported usage 累计；未覆盖所有 provider 计费差异 |
| OTel 投影生成 invoke_agent/chat/execute_tool spans，带 gen_ai 属性且确定性 | cmd: cargo test otel --lib && cargo test --test otel_cli | ✅ | JSON 投影，非完整 OTel SDK/exporter |
| OTel OTLP/HTTP JSON 导出：resourceSpans/scopeSpans、hex trace/span id、kind/status/attributes；HTTPS/loopback、不跟随重定向、非 2xx 失败、token 仅环境变量 | cmd: cargo test --test otel_export_cli | ✅ | loopback collector；非 gRPC/SDK/批量重试；属性统一 stringValue；跨平台未测 |
| Chat 适配契约：消息→TaskRequest；回复先结论+信任摘要+needs_approval+inbox_open，不泄露 schema；文件桥 task/reply/ingest spool（同 stem/同内容 SHA-256 跳过、可选 lock） | cmd: cargo test chat --lib && cargo test chat_bridge --lib && cargo test --test chat_bridge_cli | ✅ | 文件式适配器，非 Slack/Telegram/微信连接；状态上限 10k；崩溃残留锁需人工清理 |
| 团队角色策略：owner/approver/member/viewer × read/write/approve/manage + scope；未知 actor/越权/未授权拒绝；可选写入组织审计链 | cmd: cargo test team --lib && cargo test --test team_check_cli | ✅ | 策略模块，非 IdP；审计 append 与判定非原子 |
| Agent Card Ed25519 签名与信任公钥验签；私钥可写 0600 文件并从文件签名；篡改/错钥/未签名拒绝 | cmd: cargo test agent_card --lib && cargo test --test agent_card_cli && cargo test --test card_trust_cli | ✅ | 本地签名，非 A2A registry 发现/签名条目；`--sign-key` 仍会进 argv |
| 持久幂等记录：key+fingerprint→response append-only JSONL，sync 后确认，跨重启重放；冲突/半条记录处理 | cmd: cargo test idempotency --lib && cargo test --test idempotency_cli | ✅ | 去重原语；未接入所有外部写 |
| 带外 watchdog：daemon 每轮写心跳；缺失/过期/损坏非零退出；`--execute` 可显式暂停/恢复匹配进程 | cmd: cargo test watchdog --lib && cargo test --test watchdog_cli | ✅ | 默认只观察；不重启/不清理锁；Unix only |
| 运行时 kill switch：`~/.1a/KILL` 存在时写工具 PermissionDenied/kill_switch_active，只读工具可用；run/resume/surface/TUI 接线 | cmd: cargo test kill_switch --lib | ✅ | 本地文件；同 UID 可创建/删除；检查与执行有竞态；非 OS 沙箱 |
| daemon 安静时段：窗口内跳过 due 任务、保持 due、窗口后运行 | cmd: cargo test --test daemon_quiet_cli | ✅ | 固定 UTC offset；DST 由调用方处理 |
| daemon 安静时段边界唤醒：静默期睡眠取 `min(poll, 窗口剩余毫秒)`，窗口结束后立即重查 due | cmd: cargo test autonomy --lib | ✅ | 计算级单测（跨午夜/固定 offset/边界）；未做长驻 daemon 时钟实测 |
| A-Gate 本地 soak：5 cycles 均通过（300 task events / 200 memories / skills） | cmd: for i in 1 2 3 4 5; do cargo test --offline --test a_gate_soak -- --ignored; done | ✅ | 本地软件 soak，非外部 A-Gate 清单/24h 真实多 Agent |
| 自治安静时段：跨午夜窗口内主动 act/approval → defer 并报告剩余分钟；用户发起/显式批准不延后 | cmd: cargo test autonomy --lib && cargo test --test autonomy_cli | ✅ | 策略级；运行时调度/安静时段持久化未接线 |
| 误报预算：预算耗尽或误报率超限时非关键告警转 digest，critical 绕过 | cmd: cargo test alarm_budget --lib && cargo test --test alarm_budget_cli | ✅ | 策略函数；不发送/不持久化告警 |
| 升级阶梯：严格递增阈值取最高已到达台阶；resolved 不升级；宽限期不打扰 | cmd: cargo test escalation --lib && cargo test --test escalate_cli | ✅ | 只计算级别；不发送/不改状态 |
| 升级确认：`inbox ack` 只写 escalation state，幂等；ack 项保持可见但 level=none/reason=acknowledged，不投递；不改 inbox 状态 | cmd: cargo test escalation --lib && cargo test inbox_escalation --lib && cargo test --test inbox_escalate_cli | ✅ | 只停止升级，不是 inbox 终态；同 state 的多目标共享确认 |
| Inbox 升级投影/投递：`inbox escalate` 对 open/到期 snoozed 项按 `created_ms` 给出最高台阶；带 `--webhook --state` 或可重复 `--target FORMAT=URL` 时只投递新达到的 `notify`/`urgent` 台阶，终态项不出现 | cmd: cargo test --test inbox_escalate_cli | ✅ | 每次最多 50 条；多目标共享 state，任一失败不推进且重试可能重复；发送成功后才推进 watermark，崩溃窗口可重复一次；`digest` 台阶不投递；snoozed 未到期不出现 |
| daemon 自动升级投递：每个 tick 投影 open inbox 并只投递新的 `notify`/`urgent` 台阶（webhook 或可重复 `--escalate-target`）；安静时段不投递；发送失败记录并在下个 tick 重试 | cmd: cargo test --test daemon_escalation_cli | ✅ | 单 daemon/单 state；多目标顺序发送、部分失败可重复；at-least-once；不投 `digest`；跨平台未测 |
| 组织审计索引：actor/action/scope/decision 哈希链；改字段/换序/断链检测；list 尾部查看 | cmd: cargo test org_audit --lib && cargo test --test org_audit_cli | ✅ | 本地索引，非远程透明日志/IdP |
| MCP OAuth claims：audience/过期/未生效/最大 TTL/精确 scope；拒绝时不泄露 subject | cmd: cargo test mcp_auth --lib && cargo test --test mcp_token_cli | ✅ | 调用方必须先验签；不实现 issuer discovery/token exchange |
| MCP streamable HTTP：JSON/SSE 响应、Mcp-Session-Id、仅 loopback HTTP（含 IPv6 `[::1]`）、token 只读环境变量或 0600 普通文件（O_NOFOLLOW+fstat）、per-server timeout、2 MiB 响应上限；结果带 server/tool 审计元数据 | cmd: cargo test tools::mcp --lib | ✅ | 覆盖初始化/发现/调用；文件 token 拒绝符号链接/宽权限/空值/双来源；真实服务端互操作未验证 |
| MCP HTTP 会话复用：initialize 返回的 Mcp-Session-Id 必须附加到后续 notifications/initialized、tools/list 与 tools/call，缺失即 400 | cmd: cargo test tools::mcp --lib | ✅ | fake loopback server 强制校验；stdio 单进程复用；终端持久 PTY 依赖 Workshop，原生 PTY 未做 |
| MCP token exchange：RFC 8693 风格表单、RFC 8414/OpenID issuer discovery（issuer 回显校验、远程 issuer 发现端点必须 HTTPS、IPv6 loopback）、subject/client 秘密仅环境变量、HTTPS/loopback、不跟随重定向、1 MiB 响应上限、Bearer+TTL 校验；token 只写新 0600 文件，Unix 读取用 O_NOFOLLOW+fstat；MCP HTTP 可在 spawn 时自动交换、到期前 30s 主动刷新、401 时再交换一次 | cmd: cargo test mcp_exchange --lib && cargo test --test mcp_token_exchange_cli && cargo test tools::mcp --lib | ✅ | 单次 401 refresh，无重试预算/并发去重；stdout 只出 hash；真实 IdP 未验证；跨平台未测 |
| 每验证结果成本：value-report 接收验证器计数或 verification store，输出 cost_per_verified_outcome 与窗口 verification summary；含 open handoffs | cmd: cargo test value_report --lib && cargo test --test value_report_cli && cargo test --test verification_store_cli | ✅ | store 只计 clean Verified；不从 completed 推断 |
| 验证记录库：append+sync、重复 id/自验证拒绝、summary/hacked bps/时间窗口、窗口计数、Hacked/Rejected 投影 inbox review | cmd: cargo test verification_store --lib && cargo test --test verification_store_cli | ✅ | 本地 JSONL；无跨机聚合 |
| Windows GNU 交叉编译/链接：`-D warnings` 下 all-targets（含测试）check 通过；`cross-check.sh --build` 用 MinGW 链接 release `.exe` | cmd: ./scripts/cross-check.sh --build | ✅ | 本机 release 链接成功（约 32 MB）；未在 Windows 运行/ConPTY/安装包；macOS/MSVC 被 Host C 工具链阻断 |
| Onboarding 文本导入：按行写入 memory，大小/行数/行长限制，非法 kind/文件拒绝 | cmd: cargo test --test onboard_cli | ✅ | 纯文本；不脱敏；非日历/邮件导入 |
| Inbox 接受交接：handoff:<id> approved 需要 store+actor，先接受再关闭；错 actor/终态拒绝 | cmd: cargo test --test handoff_inbox_cli | ✅ | 本地；非原子（接受后 inbox 写入失败可能部分完成） |
| 组织审计 seal：Ed25519 签 entries+head_hash；追加/改 head/错钥拒绝 | cmd: cargo test org_audit --lib && cargo test --test org_audit_seal_cli | ✅ | 依赖 seal 公钥的带外信任；非远程透明日志 |
| 每日摘要：运行/计划/健康/告警/inbox open/升级级别计数/指定收件人 open handoffs/验证摘要；可单向上推 webhook 并写回执 | cmd: cargo test --test digest_cli && cargo test --test digest_notify_cli | ✅ | 汇总只读；webhook 只发不收；回执不含 URL；不含维护循环；升级计数需显式传 `--escalation-step` |
| Repair runbook：compensate/manual/errors/clean，require-clean 非零 | cmd: cargo test repair --lib && cargo test --test repair_plan_cli | ✅ | 只计划；不执行补偿/不调用 Authority |
| Repair mark：人工执行补偿后记录 compensated；failed 副作用经人工确认无残留后可清除；actor+evidence 写入账本事件；重复/错 ticket/planned 失败关闭 | cmd: cargo test --test repair_mark_cli && cargo test task_graph --lib | ✅ | 记录人工声明，不验证补偿真实执行；不执行命令/不调用 Authority |
| 单向 Inbox 通知：open items → webhook，HTTPS/loopback、拒绝凭证、不跟随重定向、空队列不发送、`--state` watermark 去重、`--log` 回执、`--dry-run`；`--target FORMAT=URL` 可多目标扇出（重复项拒绝） | cmd: cargo test notify --lib && cargo test --test inbox_notify_cli && cargo test --test inbox_notify_targets_cli | ✅ | 只发不收；不改 item 状态；多目标共享 watermark，部分失败不推进且重试可重复投递；非完整聊天传输 |
| 通知回执汇总：`notify-receipts` 读 inbox/digest JSONL 回执，统计 total/inbox/digest/last_sent/formats；坏行/未知形状失败关闭，缺失文件为空 | cmd: cargo test --test notify_receipts_cli | ✅ | 只读；不验证真实送达；跨平台未测 |
| 信任披露清单：能力计数/限制/数据政策/评测披露/本地验证摘要；JSON + Markdown + 静态转义 HTML；audit 固定 not_audited；HTTP 公开只读 `/v0/trust-center(.html)` | cmd: cargo test trust_center --lib && cargo test --test trust_center_cli && cargo test http_surface --lib | ✅ | 本地文件/loopback HTTP，非托管服务/外部审计；HTML 不含脚本，联系信息按文本转义 |
| 事故披露：唯一安全 id、open/resolved 时间一致、http(s) link 校验；按开启时间倒序渲染，HTML 转义 | cmd: cargo test trust_center --lib && cargo test --test trust_center_cli | ✅ | 调用方提供事故 JSON；不独立验证事故真实性；不是托管事故页 |
| eval 生成评测披露：suite/version/passed/total + 报告 SHA-256，可被 trust-center 读取 | cmd: cargo test --test eval_disclosure_cli | ✅ | 披露绑定本地报告；不证明评测真实/外部复现 |
| 单位经济：contribution/net/gross margin bps/每结果成本/break-even | cmd: cargo test unit_economics --lib && cargo test --test unit_economics_cli | ✅ | 调用方提供价格/成本；不内置定价 |
| 基准门禁：完成率/crash-free/replay bps；crash 不计入完成或 replay；空/超限/非法阈值失败关闭；JSONL 行号错误；CLI 失败非零 | cmd: cargo test benchmark --lib && cargo test --test benchmark_cli | ✅ | 只计算调用方样本，不是 8h runner；默认阈值为计划目标，不是证据 |
| 文本附件：最多 5×1 MiB/总 4 MiB；仅普通 UTF-8 文本；拒绝 NUL/二进制/符号链接；作为 untrusted data 注入并记录 SHA-256 | cmd: cargo test attachment --lib && cargo test --test attachment_cli | ✅ | 不脱敏；不证明模型抵御提示注入；图片/屏幕/语音未实现 |
| Agent Card well-known 公开发现路由与 /v0/agent-card 同 payload | cmd: cargo test http_surface --lib | ✅ | 发现入口；非 A2A registry 发现/签名条目 |
| 安静时段持久化：onboard --quiet-hours 写入 preference，UserModel 读取并回显；autonomy-for 读取域档位+安静时段 | cmd: cargo test user_model --lib && cargo test --test onboard_cli && cargo test --test autonomy_cli | ✅ | 偏好视图；无窗口结束唤醒/调度 |
| Agent Card 信任锚：受信/吊销公钥库；未受信/已吊销/篡改拒绝 | cmd: cargo test card_trust --lib && cargo test --test card_trust_cli | ✅ | 本地信任锚，非远程密钥发现 |
| 团队交接：必须有 evidence、from≠to；仅 addressee 可接受；from/to 可 cancel；终态不可再转换；list 可按 open/to 过滤 | cmd: cargo test handoff --lib && cargo test --test handoff_cli | ✅ | 本地交接；无实时共享上下文/IdP |
| 交接投影：open + to=recipient → inbox review，`handoff:<id>` 去重；reconcile 关闭 accepted/cancelled 项 | cmd: cargo test inbox --lib && cargo test --test handoff_inbox_cli | ✅ | 仅投影；通知/实时上下文未实现 |
| daemon 定期 reconcile 交接项：terminal handoff → approved/dismissed，每个 tick 执行，安静时段跳过 | cmd: cargo test --test daemon_reconcile_cli | ✅ | 需显式三个 `--reconcile-*` flag；仅本地文件；跨平台未测 |
| daemon 告警投影：`--alert-log` + `--alert-inbox-store` 每个 tick 把新 alert 投影进 inbox，按 item_id 去重 | cmd: cargo test --test daemon_alert_inbox_cli | ✅ | 只投影；不改 alert 日志；需显式 flag；跨平台未测 |
| daemon 目标投影/清理：`--goal-journal` + `--goal-inbox-store` 每个 tick 把逾期 goal/commitment 投影进 inbox，并 dismiss 条件已消失的 `attention:` 项；按 item_id 去重 | cmd: cargo test inbox --lib && cargo test --test daemon_goal_inbox_cli | ✅ | 只投影；不改 goal journal；dismiss 后条件重现不会自动重新添加（item_id 稳定）；需成对 flag；跨平台未测 |
| daemon 轻量定期 digest：`--digest-webhook` + `--digest-every-seconds 60..=86400` 发送 runs/schedules/alerts/inbox_open/升级级别，可选 journal/inbox store 与 `--digest-state` 持久化计时 | cmd: cargo test --test daemon_digest_cli | ✅ | 无 state 时重启立即发送一次；不替代完整 `digest`；发送失败只记录；升级计数需 `--digest-escalation-step` + inbox store；跨平台未测 |
| Tool search 按名称/前缀/描述/参数名确定性排序；run --tool-query 限制并记录模型可见工具 | cmd: cargo test tool_search --lib && cargo test --test tool_search_cli && cargo test --test tool_query_cli | ✅ | top 8 + always-on；词法而非语义；无 query 时全量 |
| 读权限关闭时终端执行工具不暴露给模型 | test: write_tools_are_hidden_without_write_permission | ✅ | 工具注册表按权限过滤 |
| 终端工具可在真实 PTY 会话中创建、执行并销毁 | cmd: cargo test --offline --test terminal_tools_live -- --ignored | ⚠️ | 需本机 skynet-term-sessiond 与空闲会话槽位；本次 128 会话上限已满，测试返回 `session_limit`，无法在当前 daemon 重跑；默认测试集跳过 |
| 1A 在写权限下用终端工具启动 copilot 并读取 GitHub 登录状态 | file: docs/evidence/terminal-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机验收；gh 已登录，未做交互式 device-code 登录 |
| 本地 CLI/TUI 默认允许执行，`--read-only` 关闭 | test: default_run_allows_execution_and_read_only_blocks_it | ✅ | CLI 层；使用 mock provider |
| TUI 默认开启执行，无需手动 `/permissions write on` | file: docs/evidence/terminal-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；tmux PTY；journal `allow_write=true` |
| 文件写入/编辑工具受写权限、根目录、符号链接和覆盖保护 | cmd: cargo test --offline --lib tools::files | ✅ | 原子替换；路径与符号链接有独立测试 |
| fetch_url 只读公网 HTTPS，拒绝私网/非文本/超大响应 | cmd: cargo test --offline --lib tools::web | ✅ | 纯函数 + 权限测试；实机抓取见 docs/evidence/web-2026-11-09 |
| 1A 可用 fetch_url 完成真实网页读取 | file: docs/evidence/web-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；example.com |
| 1A 可用 web_search 返回真实搜索结果 | file: docs/evidence/web-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；Bing RSS；结果可能本地化/限流 |
| MCP stdio client 可发现并调用工具 | cmd: cargo test --offline --lib tools::mcp | ✅ | fake server 契约测试；写工具权限门控 |
| 1A 可通过 `--mcp-config` 调用真实 MCP 工具 | file: docs/evidence/mcp-2026-11-09/README.md | ✅ | 单次 DeepSeek 实机；本地 fake echo server |
| 子 Agent 可隔离运行、回传结论并限制深度/总扇出；只接受 completed 子运行；存在子 journal 时必须 audit 通过；请求的 write/network 只与父权限取交集 | cmd: cargo test --offline --lib tools::subagent | ✅ | fake program；扇出为单进程实例上限；子 journal 缺失时不审计；跨进程扇出未限制 |
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
| 模型读取实时工具、参数、权限、模型和预算，不再使用静态能力目录 | test: describe_self_reports_the_live_registry | ✅ | `describe_self` 读取权限过滤后的 `ToolRegistry`；能力问题跳过无工具快聊；执行模式下的能力问题不会被只读授权短路 |
| 模型可在循环中请求缺失工具；写/网络权限不足时明确拒绝 | cmd: cargo test --offline --lib request_tools_ | ✅ | `request_tools` 校验未知工具、写权限和网络权限；只把允许的 schema 加入下一轮请求；`ToolsRequested` 写入 journal |
| 已完成答案若与自身任务需求矛盾，最多再规划 2 次，仍不足降级为 `needs_evidence` | test: evidence_replan_is_bounded_and_downgrades_to_needs_evidence | ✅ | 每轮写 `AnswerVerificationFailed/Exhausted`；无死循环；快聊路径同样按自身评估升级 |
| 不可修复的模型答案转为可审计 `needs_evidence`，不声称完成 | test: irreparable_answer_output_becomes_needs_evidence | ✅ | 原始无效响应与修复请求留在 journal；不执行动作、不声称成功 |
| 策略记忆只写 Draft，不自动注入；内容不含用户原文 | test: strategy_draft_records_tool_sequence_without_user_text | ✅ | 复用 `ExperienceStore` 的 review/activate/rollback；只记录模式、目标哈希、工具序列、结果；相同草案去重 |
| 100 题智能闭环评测清单固定 | test: agent_loop_suite_has_one_hundred_fixed_cases | ✅ | 自我介绍、最新/本地、追问、简单聊天、高风险各 20；真实模型报告与原始 journal 另见 evidence |
| 空查询的 `search_files` 列出文件名，不读取内容 | test: empty_query_lists_files_without_reading_contents | ✅ | 只返回 read root 内的路径，跳过符号链接、生成目录和控制字符；不读二进制内容 |
| 交互式 TUI 默认不限制步数、工具调用、模型尝试和运行时长 | test: zero_budgets_do_not_stop_a_long_run | ✅ | `0`=不限；17 次工具调用后仍完成；后台 run/daemon/子 Agent 仍有界；`zero_budgets_are_valid_and_mean_unlimited` 覆盖会话配置校验；单次模型 HTTP 调用 300 秒上限 |
| token 压缩在达到阈值时触发 | test: token_compaction_triggers_before_the_hard_limit | ✅ | 旧 `compact_messages_at` 字符接口保留兼容；journal 同时记录 token/字符 |
| Master 状态卡覆盖理解/规划/批准/分配/启动/Worker/收集/验收/汇报 | test: activity_taxonomy_is_bilingual_and_color_coded | ✅ | 持久卡片；下一步命令写在 footer |
| 工具调用和结果实时写入主会话工作日志 | test: work_entries_use_a_bilingual_label_in_the_transcript_log | ✅ | journal 增量驱动；状态与详情用 ` · ` 分隔；真实 PTY 证据 `docs/evidence/tui-2026-11-10/work-log.txt`；Bash 输出有长度上限 |
| 整个 TUI 大背景为暗绿色，为输入面板 80% 亮度且更暗 | test: terminal_backdrop_is_darker_green_than_the_input_panel | ✅ | 大背景 `#06100A`、输入面板 `#08140D`；欢迎页、主会话和命令面板共用大背景 |
| 欢迎页 `1` 保持亮绿，`Agent`/底部文字深绿，域名亮绿 | test: welcome_logo_highlights_the_s_and_paints_the_rest_dark_green | ✅ | 真实 PTY：`1`=`#8aa652`，`Agent`/说明=`#8b966f`，`SKYNET.CLUB`=`#8aa652`，阴影 `#4a5930`；退出 banner 使用同一组值 |
| TUI 输入框/状态/模型/模式共用深绿围栏，token 指标嵌在顶部边框右侧 | test: composer_and_panel_text_share_one_fence | ✅ | 暗绿背景/边框/文字整体压暗 50%、白色输入；底部模型/模式；`T` 亮绿加粗；上下文只在顶部进度条 |
| 窄终端时 token 指标折入围栏内下一行，进度条减半，不再截断 | test: narrow_terminal_folds_metrics_inside_the_same_fence | ✅ | 72 列真实 PTY 证据 `docs/evidence/tui-2026-11-10/input-panel-folded.txt`；进度条 20→10 格，百分比不变 |
| 用户消息整行使用会话框背景 `#08140D`、白色文字 | test: user_message_and_answer_are_separated_by_an_empty_gap_row | ✅ | 真实 PTY 采样用户行 bg=08140d；回答前的空白行不继承该背景 |
| 右键粘贴仅在输入框内部触发 | test: right_click_paste_is_limited_to_the_composer | ✅ | 输入框矩形命中测试；真实 PTY：框外右键无提示，框内右键出现“已从剪贴板粘贴”；Ctrl+V 不受限 |
| 工作日志中的本地路径与 URL 为亮绿色 | test: work_log_colors_links_bright_green | ✅ | 绝对/相对路径与常见扩展名；URL 仅 http(s)；统一使用 WorkLink 亮绿色 |
| 工作日志动作名称和 `•` 为亮绿，详情与 `⎿` 结果为深绿且不加粗 | test: work_log_uses_bright_green_labels_and_deep_green_details | ✅ | 80x20 TestBackend：动作名如“模型思考 Thinking”为 `#8aa652`；详情/结果 `#8b966f`；均无 BOLD |
| 文件编辑在工作日志显示 `✓ +N -M` 和最多 12 行 diff，`+` 亮军绿、`-` 红 | test: work_log_colors_added_and_removed_diff_lines | ✅ | 文件工具把 added/removed/diff 写入 ToolObservation.metadata；真实任务把 `alpha` 改成 `beta` 后工作日志出现 `编辑 Edited`、`✓ +1 -1`、`-alpha`、`+beta` |
| `Ctrl+Q` 连按两次退出；`F10` / `Alt+Q` / `/quit` 是外层占用时的兜底 | test: maps_keys_and_control_shortcuts | ✅ | PTY：`\x11\x11` 与 F10 两次均 exit 0；底栏显示 `F10 退出`；`Enter`/`Esc` 取消 |
| TUI 主题、欢迎页和退出页与 CLI `help` 军绿一致 | test: tui_palette_matches_the_cli_military_green | ✅ | `#708a41 / #8aa652 / #8b966f / #687358 / #4a5930`；退出 banner ANSI 同步；错误红/警告黄保留 |
| 最终答案使用“结论 Conclusion”`#4a5930` 边框、`#8aa652` 加粗标题、白色正文 | test: final_answer_is_wrapped_in_a_conclusion_fence | ✅ | direct/structured 共用；围栏内上下各留一空行；真实 PTY 证据 `docs/evidence/tui-2026-11-10/conclusion-fence.txt` |
| 结论正文支持 Markdown 标题、列表、任务框、分隔线和链接高亮 | test: conclusion_markdown_renders_headings_bullets_and_bold_labels | ✅ | `##` 粗体标题；`- ` 项目符号；`1.` 有序行；`- [x]`→☑/☐；`---`→分隔线；`**重点**` 加粗标签；URL/路径使用链接色；正文不残留 `**` |
| 结论正文支持列宽对齐的 Markdown 表格 | test: conclusion_markdown_renders_an_aligned_table | ✅ | 表头、分隔行和数据行等宽；超宽单元格截断并补空格；窄终端按行折行 |
| 系统提示要求多维度答案使用短段落/列表/对比表，避免一整段长文 | test: system_prompt_carries_the_occam_style_laws | ✅ | 新增 `Format for scanning` 规则；Markdown 允许且优先用于提升可读性 |
| 结构化推理区块全部深绿，只有结论答案为白色 | test: structured_sections_are_deep_green_while_the_answer_stays_white | ✅ | 主张/依据/假设/约束/未知/冲突/翻转条件的标题、标签、项目符号与正文均为 `#8b966f`；围栏内答案为白色 |
| `/` 命令抽屉的指令名称和说明均为亮绿色 | test: command_drawer_uses_bright_green_entries | ✅ | 名称加粗、说明不加粗；窗体背景 `#0A1F12`，选中行 `#143D24` |
| 所有 TUI 弹窗（详情/命令面板/抽屉/模式菜单）使用绿色系背景 | test: interaction_overlays_use_green_surfaces | ✅ | 背景 `#0A1F12`、选中 `#143D24` 均为绿系；详情边框亮绿；真实 PTY 采样确认 |
| `/` 命令抽屉可搜索 48 条提示词补充 | test: prompt_supplements_are_featured_and_searchable | ✅ | 按名称/分类/触发/用途搜索；中文空查询含 3 条精选；输入 `//` 浏览全部 48 条；English 空查询列出 14 条有英文原文的卡片；真实 PTY `/验收` 命中 |
| 选中提示词插入正文并保留变量，不自动执行 | test: prompt_supplement_completion_fills_the_composer | ✅ | 输入框填入模板，`{{变量}}` 留给用户；真实 PTY 确认 |
| 已写任务后输入 `//` 打开原文库并追加到任务 | test: prompt_drawer_opens_after_a_task_and_merges_the_source_text | ✅ | 原文追加到原任务，保留完整正文；不执行 |
| 任务内 `//关键词` 搜索，Esc 恢复原任务 | test: prompt_drawer_searches_inside_a_task_and_escape_restores_it | ✅ | 抽屉只显示提示词；Esc 去掉 `//` 查询 |
| 空输入框输入 `//` / `；；` / `;;` 打开三栏提示词补齐窗 | test: double_slash_opens_the_prompt_picker_and_source_category | ✅ | 左侧类目（无“全部”）、中间提示词、右侧详情；默认加目标 5 条；原文库是左侧一个类目；`→` 进入列表、`Enter` 插入、`Esc` 关闭 |
| `//` / `；；` 紧贴正文也能触发，URL 的 `://` 不误触；`→` 进列表、`Enter` 应用 | test: prompt_marker_accepts_adjacent_and_semicolon_triggers | ✅ | `做个网页//`、`做个网页；；`、`做个网页;;` 均打开；`https://` 与 `file:///` 不打开；焦点/插入由 `prompt_picker_right_enters_list_and_enter_inserts` 覆盖 |
| 原文库插入完整原文，不截断、不改写 | test: double_slash_opens_the_prompt_picker_and_source_category | ✅ | 中文插入完整中文直译；English 插入完整英文原文；只插入正文，作者/主题/分类/来源链接只留在抽屉行；测试断言输入包含整段 `translation`/`original` 且不含标题与参考行 |
| 同一段原文重复插入不会重复堆叠 | test: inserting_the_same_source_twice_does_not_duplicate_it | ✅ | 输入框已包含正文时跳过并提示，保留原任务意图 |
| 原文库按作者+主题归组并保留源表记录 ID | test: prompt_drawer_without_a_task_lists_the_whole_source_library_by_category | ✅ | 32 组；Think Before Coding 含 6 条片段；生成脚本保留 `record_ids` 与来源链接 |
| 原文库每组与飞书表逐行原文完全一致 | test: source_groups_keep_every_table_row_complete | ✅ | 重新解析快照 ndjson，逐组比对 `original`/`translation` 拼接结果与行数，防止导入截断或改写 |
| 提示词补齐窗渲染左侧类目、中间列表和右侧详情 | test: prompt_picker_renders_categories_and_list | ✅ | 120x40 TestBackend：`提示词补齐`、`加目标 (5)`、`原文库 (32)`、提示词名、详情正文与 `←→ 类目/列表/详情` 均可见；无“全部”类目 |
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

## 0.98 安全与可靠性

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| setup 在全新 home 上 `--check` 只读且不写状态 | test: setup_check_on_fresh_home_is_read_only | ✅ | 隔离 HOME；错误/provider 路径另有测试 |
| deterministic setup 强制真实 identity `offline-v1` | test: setup_deterministic_is_idempotent_and_status_ready | ✅ | 传错 `--model` 会被纠正，status/run 可用 |
| 不可逆浏览器点击必须经持久动作卡，且只消费一次 | test: irreversible_click_requires_and_consumes_an_action_card | ✅ | 真实 Chrome；假“已批准”按钮/跨域链接/下载链接同样被门禁 |
| 动作卡可由 CLI 列出、批准、拒绝，无 TTY 时拒绝批准 | test: approvals_are_listed_approved_and_denied_from_the_cli | ✅ | `browser approvals/approve/deny` |
| MCP modern 2026-07-28 无 session，发 `Mcp-Method`/`Mcp-Name` 头 | test: modern_stateless_mcp_negotiates_and_sends_headers | ✅ | 合成 HTTP server |
| MCP 工具定义变化时 fail closed | test: tool_hashes_are_canonical_and_pin_files_round_trip | ✅ | 0600 pin 文件 |
| MCP MRTR `input_required` 不自动作答、不泄露 requestState | test: mrtr_input_required_fails_closed_without_echoing_state | ✅ | URL 只展示 origin |
| MCP Tasks 仅在显式启用时轮询，完成/输入/禁用路径符合契约 | test: tasks_poll_to_completion_when_enabled | ✅ | `tasks/update` 未实现，输入即 fail closed |
| MCP modern 参数头按 `Mcp-Param-*` 镜像，并与 body 一致 | test: mirrors_x_mcp_header_parameters_to_a_real_http_server | ✅ | 真实 conformance HTTP server 校验 header/body |
| MCP 客户端可连接 3 个真实 stdio 配置（modern/legacy/prompt） | test: calls_three_real_mcp_server_configurations | ✅ | 本地独立 server 代码库；非远程 OAuth |
| `x-mcp-header` 注解只接受安全标量并做 Base64 编码 | test: x_mcp_header_annotations_are_validated_and_scalarized | ✅ | 单元测试 |
| CIMD client metadata URL 校验并作为 client_id | test: cimd_document_is_validated_and_used_as_client_id | ✅ | 不代替 AS 侧抓取/校验 |
| 技能 blind comparator 隐藏 A/B 并给出 Wilson 95% lift | test: blind_comparator_hides_arms_and_reports_a_significant_lift | ✅ | 输出只存 hash |
| held-out split 确定且两侧非空 | test: held_out_split_is_deterministic_and_both_sides_non_empty | ✅ | 小套件 fail closed |
| active 技能锁文件可检测漂移 | test: lock_pins_active_versions_and_detects_drift | ✅ | 双向精确匹配 |
| 技能脚本沙箱无宿主 home/网络，且拒绝越界路径 | test: runs_a_script_without_home_network_or_writes | ✅ | bwrap；bwrap 缺失 fail closed |
| 100 次真实子进程崩溃后同一槽位不会被重复 claim | test: one_hundred_hard_crashes_never_duplicate_a_claimed_slot | ✅ | 子进程 claim 后 `_exit(137)` |
| 100 个槽位 claim/reconcile 无重复 | test: hundred_claimed_slots_reconcile_once_without_duplicates | ✅ | 账本矩阵 |

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
| 并列方案不超过 3 个且给出默认推荐 | cmd: cargo test reframe --lib | ⚠️ | 2..=3 选项上限已由 validate 强制；默认推荐字段/校验未实现 |
| 未决定时给出可执行的方案菜单 | test: no_decision_prints_a_menu_and_writes_nothing | ✅ | CLI 层；TUI 未接；方案菜单按设计无默认项 |
| /reframe 可被 Ctrl+C 取消且不残留提议 | cmd: python3 scripts/pty_reframe_cancel.py | ✅ | PTY，本地黑洞端点；取消后会话仍可用 |
| 重构失败时给出可读错误且不泄露端点 | cmd: python3 scripts/pty_reframe_failure.py | ✅ | PTY，连本地关闭端口；失败后会话仍可用 |
| TUI 内可完成需求重构与决定 | cmd: python3 scripts/pty_reframe.py | ✅ | PTY 真实输入，断言提议面板与决定落库 |
| 每轮结束提供带默认项的动作菜单 | cmd: python3 scripts/pty_next_actions.py | ✅ | 真实 PTY 驱动 TUI，断言面板出现且默认项有标记 |
| 回答语言与请求一致 | file: docs/evidence-answer-style-v3.md | ⚠️ | 全新 14 个任务 0 条语言错配；关掉语言条款的对照曾出现 1 条 |
| 菜单项只来自已注册命令 | test: every_entry_carries_a_real_command_name | ✅ | 只证明在注册表内；`implemented` 标志恒为 true，见下方缺陷 |
| 真实语料中"手段类请求"的占比 | 无 | ❌ | 75 条 journal 中真实请求约 10 条，样本不足 |
| 需求重构带来的成本下降 | 无 | ❌ | 未做对照实验 |
| 目标 achieved 需要已验证 run 的独立验证记录；缺失/run 错配/自验/Rejected/Hacked 均失败关闭，旧 goal 日志仍可 replay | cmd: cargo test --test goal_cli && cargo test --lib goal | ✅ | 本地 VerificationStore JSONL；验证器独立性由 store 校验，不是签名审批信封；不读取答案正文 |
| Track Q 最终 99/100：预算停止原因跟随答案状态；修复后高风险答案仍过 re-plan；日期 rubric 等价匹配；7 例质量/安全子集 7/7 | file: docs/evidence/0.99-acceptance/agent-loop-v1-final-2026-11-17/report.json | ✅ | 单次 DeepSeek 运行；残余 chat-19 简单聊天 2 次 provider call；非统计稳定性 |
| 默认满血：本地 CLI/TUI 写入与网络开启；`--no-network` 关闭网络；TUI `/permissions network off` 仅影响下一次运行；模型不能自我提权，网络拒绝错误与 `describe_self` 给出准确命令；clarify 选项必须是人类可执行的 | cmd: cargo test --test cli_e2e tui_network_permission_defaults_on_and_no_network_disables_it && cargo test --lib system_prompt_does_not_let_the_model_grant_permissions && cargo test --lib self_model_reports_only_available_tools_and_live_budgets | ✅ | 不自动改权限；仍需人类输入命令；默认开网扩大提示注入外发面，O1/O2 仍为残余风险 |
| `/setting` 统一设置入口（`/settings` 别名）提供 show/set、8 套 TUI 主题和两档恢复；主题存 `appearance.theme`，全局恢复先备份并要求 confirm | cmd: cargo test --bin first-principles-agent setting_ && cargo test --lib tui_theme::tests && cargo test --test cli_e2e tui_setting_menu_shows_values_and_switches_theme | ✅ | 主题仅 TUI，CLI help 不变；主题为静态表，无自定义主题编辑器 |

## 覆盖率

本表逐条覆盖的条目数，以 `scripts/verify-claims.sh` 的输出为准——不在文档里写死，
避免出现第二个会漂移的数字。README 的 `## Status` 能力清单尚未逐条映射。

**未映射不等于不成立，也不等于成立——只等于没人核对过。**
