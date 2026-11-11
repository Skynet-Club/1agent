# Release Notes — 0.96.1 TUI、上下文与提示词库

[简体中文](RELEASE_NOTES.md) · [English](RELEASE_NOTES.en.md)

Date: 2026-11-11  
Status: **R1 public pre-release (patch), not production-stable**

GitHub pre-release: <https://github.com/Skynet-Club/1agent/releases/tag/v0.96.1>

## 产品是什么

1Agent 是本地优先、可审计、可解释的 Super-Agent 运行时。它把任务、证据、授权、执行、验收和审计做成一条可回放的链路：先说清楚为什么做，再决定做什么；能分析，接上受控执行底座后也能执行；每一步都留下可验证的记录。当前是 R1 候选版，不是生产稳定版。

## 版本定位

0.96.1 是 0.96.0 的补丁版本，集中改进 TUI 可读性、上下文预算、提示词补充和答案格式。它**不包含** 0.97 Reliable Loop 的流式输出、并行只读工具、增量 journal、崩溃恢复等范围。

## 本版重点

### 1. 答案可读性

- 结论围栏内支持 Markdown 层级：`##` 小标题、有序/无序列表、`- [x]` 任务框、`**重点**` 加粗标签、`---` 分隔线、链接色 URL/本地路径、按列宽对齐的表格。
- 系统提示要求：先给一句结果，再用短段落、列表或对比表组织多维度答案；简单答案仍保持一段，避免过度结构化。
- 真实 `deepseek-flash` canary 已返回“一句总结 + 能力表 + 三条要点 + 一个带默认项的问题”，不再是一整段长文。
- “参考：https://...” 这类中文紧贴 URL 的写法也能识别为链接。

### 2. 提示词库

- `/` 仍搜索 48 张精选中文卡片；空查询显示 3 条精选。
- `//` 打开完整原文库：飞书表 56 条逐字原文按“作者 + 主题”归为 32 组，并按共性分类打标签。快照与导入脚本：
  - `docs/evidence/prompt-sources-2026-11-11.ndjson`
  - `scripts/import-prompt-sources.py`
- 插入只包含完整正文：中文模式为完整中文直译，English 模式为完整英文原文；不截断、不改写、不带作者/主题标题，也不带参考链接。作者、主题、分类和链接只显示在抽屉行。
- 宽终端右侧预览选中条目的完整正文、作者、分类、片段数和字数；`Enter` 才插入。
- 同一段正文已在输入框时，再次插入会跳过。
- 新增 `/config set language zh|en`：原文库两种语言都可用；`/` 的英文精选只列出有逐字英文原文的 14 张卡片，无英文来源的卡片隐藏，不用中文回退。
- 完整性门禁：`tests/prompt_sources.rs` 重新解析 56 行快照，逐组比对 `original` / `translation` 拼接结果和行数；任何截断或改写都会失败。

### 3. 上下文与预算

- 上下文预算改为 token 计量：默认 1,000,000 tokens，达到 786,432 自动压缩；`--max-context-tokens` 取代 `--max-context-chars`。
- 旧字符预算在读取时迁移：旧默认 100,000 字符映射为 1M tokens；自定义值使用 `ceil(chars / 3)`；旧 session/task JSON 仍可读。
- TUI、`/status`、`/context` 报告 token；provider 返回 `usage.input_tokens` 后以它为准。
- 交互式 TUI 默认不再限制步数、工具调用、模型尝试和运行时长（`0` = 不限），`/config set` 接受 `0`；状态栏与 `/config` 显示“不限”。
- 后台 `run`、daemon 和子 agent 仍保持有限预算；交互式 TUI 每次 provider HTTP 调用保留 300 秒传输上限。
- 移除隐藏上限：运行时 provider 尝试硬夹（原 16 次）和 TUI 步数/工具校验上限（原 64）。

### 4. TUI 视觉与交互

- 整个 TUI 使用暗绿背景 `#06100A`；输入面板 `#08140D`、边框 `#3F644B`、面板文字 `#487748`、输入文字白色。
- 输入框顶部边框显示状态、步骤和 token/缓存/上下文指标；窄终端指标折入围栏内。底部边框显示运行状态、模型和模式。上下文条使用 `─` 格（宽屏 20 格，窄屏 10 格）。
- 欢迎页：`1` 亮绿，`Agent`/说明深绿，`SKYNET.CLUB` 亮绿。
- 所有弹窗（`/details`、`/help`、`/` 抽屉、`Ctrl+T`）统一绿色表面 `#0A1F12`、选中行 `#143D24`、边框 `#3F644B` 和亮绿标题。
- 工作日志：动作名称、`•` 和链接亮绿；详情与 `⎿` 结果深绿 `#487748` 且不加粗。`第 N 步` 统计实际模型请求数；`Ctrl+O` 查看运行详情；Bash 结果带有限长度输出预览。
- 运行状态为两行动作卡，按 run journal 显示中英双语阶段、模型步骤和极简围栏；Master-Agent 持久状态卡覆盖理解、规划、批准、分配、启动、Worker 运行、收集、验收和汇报，并带下一步命令 footer。
- 用户消息使用输入面板背景和白色文字；右键粘贴限定在输入框内。
- Composer 支持整框鼠标拖拽选择；编辑针对 stale/mid-grapheme 光标加固，输入上限 64 KiB；新增确定性编辑 fuzz 和 PTY 粘贴/退格/空格 fuzz。TUI panic 会恢复终端并追加到 `$HOME/.1a/tui-crash.log`，瞬时输入读取错误会重试。
- 最终答案围栏为 `╭─ 结论 Conclusion ... ╮`：深绿边框、亮绿加粗标题、白色正文；结构化“主张/依据/假设/约束/未知/冲突/翻转条件”使用深绿 `#487748`，只有结论正文为白色。
- 状态/工作标记改为更小的 `•`；正常活动使用一个青色强调，绿/黄/红只用于完成、等待/重试和阻塞结果。

## 下一版本：Super-Agent 0.97–1.0

- **0.97.0 Reliable Loop**（目标窗口 2026-12-22）：长时程可靠闭环——流式输出、并行只读工具、增量 journal、崩溃恢复/断点续跑、验证门、memory MVP、沙箱、结果先行的回答契约。
- **0.98.0 Real World**（2027-02-02）：浏览器写操作 / computer-use、MCP 扩展、技能学习、常驻 MVP。
- **0.99.0 Multi-Agent Organization**（2027-03-16）：Master/Worker 可靠性、成本治理、市场信任、外部审计准备。
- **1.0.0 Trusted Super-Agent**（2027-05-11）：外部审计、SDK/API、团队协作、跨平台、生态。

日期是目标窗口，不是无条件承诺；指标门未过，不发布，不宣称 Super-Agent。

## 验证

- `release-check.sh`：fmt、`clippy -D warnings`、全量测试、HTTP e2e、ci、TUI 交互，包含 400 次混合编辑。
- `verify-claims.sh`：claims 表结构通过；具体 verified/partial/no-evidence 数量以运行输出为准。
- 真实 PTY 证据位于 `docs/evidence/tui-2026-11-10/`；答案格式另做了真实 provider canary。

## 已知边界

- 仅发布 Linux x86_64；Windows 构建未发布。
- 0.96.1 仍是 R1 公开预发布版，不是生产稳定版；接口、配置和数据格式仍可能变化。
- 外部副作用（提交、推送、发布、花钱等）仍由授权/Policy 门控，不会静默执行。
- 原文库按“作者 + 主题”合并同一来源的片段，不是整篇文章阅读器；需要单行原文时请使用飞书表原链接。
- English 精选层次有 14 张卡片；`格式锁定`（Riley Goodside）和 `写成脚本`（Karpathy）暂未补英文原文，因此不在 `/` 的 English 模式出现。

## 发布资产

- `1agent-0.96.1-linux-x86_64.tar.gz` — Linux x86_64 包；
- `SHA256SUMS` — 归档与 Linux provenance 校验和；
- `linux-x86_64.json` — Linux 构建 provenance。
