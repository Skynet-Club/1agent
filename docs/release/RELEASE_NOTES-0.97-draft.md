# Release Notes — 0.97.0 Reliable Loop（候选稿）

[简体中文](RELEASE_NOTES-0.97-draft.md) · [English](RELEASE_NOTES-0.97-draft.en.md)

Date: 2026-11-14

Status: **release-candidate draft, not published; local gates and the Linux
package are being finalized**

Pending: third-party A-Gate/W-Gate/security certification, a human blind
review (the archived answer-quality labels are AI/bulk-written), macOS P-Gate,
a rebuilt clean public history, and the final package confirmation.

> This draft is not a release announcement. A metric marked “pending” is not
> zero; the evidence does not exist yet.

## 产品是什么

1Agent 是本地优先、可审计、可解释的 Super-Agent 运行时。它把任务、证据、
授权、执行、验收和审计做成一条可回放的链路：先说清楚为什么做，再决定做
什么；能分析，接上受控执行底座后也能执行；每一步都留下可验证的记录。

0.97 仍是 R1 预发布线，不是生产稳定版，也不是已经验收的 Super-Agent。

## 版本定位

0.97.0 “Reliable Loop” 的重点是**长时程可靠闭环**，不是增加更多功能：

- 流式输出与首 token 计时；
- 并行只读工具，journal 顺序确定；
- 增量 journal 读取与 1M token 解析回归；
- checkpoint 差分与模型请求差分，降低 journal 体积；
- 崩溃恢复/断点续跑与 2h 注入 soak；
- Clarify 批量面板与模型主动追问工具；
- TUI 可读性、错误恢复提示与 transcript 缓存；
- 可观测门、真实任务套件、回滚与审计材料。

0.98–1.0 的 computer-use、自动重启、多 Agent 组织、外部审计与跨平台
P-Gate 不在本版本。

## 本版重点

### 1. 可靠循环与 journal

- `--stream` 的首次 provider 尝试记录 `StreamFirstToken`，`metrics` 输出
  TTFT p50/p95；非流式运行没有样本。
- 只读工具可并行执行，但 journal 中的调用/观察顺序保持确定，审计仍能
  逐对检查。
- 增量 journal 读取只解析游标后的完整行，未换行尾部保留；4.7 MB
  journal 全量解析 31.8 ms，增量 p95 9.4–16.1 µs。
- checkpoint 差分：首个 checkpoint 全量，后续只写新增消息；100 步
  checkpoint 事件 115 KB，全量估算 2.99 MB。
- 模型请求差分（ADR-0122）：首个请求全量，后续 `ModelRequestDelta` 只写
  未变前缀之后的消息；100 步 echo run 模型请求事件 74.9 KB，全量估算
  3.03 MB（2.5%）。`ModelRequestChain` 重建请求，audit 拒绝孤立 delta
  和前缀越界。
- 2h 崩溃注入 soak：28,552 cycles / 7,200.20s，exit 0；完成副作用不重复，
  中途调用或无 checkpoint 的 journal 失败关闭。
- 回滚演练：当前二进制可读 0.96.1 journal；0.96.1 发布二进制不能读含
  `checkpoint_delta`/`model_request_delta` 的 0.97 journal。

### 2. Clarify：先问清楚，再执行

- `/clarify` 批量面板：1–5 个阻断问题同屏，逐题锁定，内联 `Other`，一次
  确认，支持取消与 300 秒超时。
- 结果状态：`submitted|cancelled|timed_out|undelivered`；答案状态
  `answered|skipped|unanswered`；未回答不会让默认答案失效。
- 交互式 TUI 注册模型主动 `clarify` 工具；行/headless 运行不注册。真实
  PTY 已验证模型发起 → 面板 → 工具观察 → 最终答案。
- 限制：问题历史尚未跨会话持久化；鼠标/IME 未覆盖。

### 3. TUI 可读性与操作反馈

- unslop 设计审计：正文对比度 6.0:1、边框 3.8:1；只保留一个绿色强调与
  链接角色；短终端使用紧凑 wordmark；空闲状态与指标降噪；帮助面板显示
  当前位置；空 composer 给出提示。
- 工作日志每步只一行 `模型请求`，显示 provider/model 与新增/总消息数及
  角色分布，去掉无意义的上下文 token/可用工具数；工具结果按 call id 紧贴
  调用；常见失败中文化并保留 URL；结束原因中文化；会截断的宽表改为堆叠行。
- `/clarify` 全屏面板、错误恢复提示、transcript 投影缓存（200 条长
  transcript release 帧 p95 1.26 ms）。
- 行模式与交互模式共用同一错误 hint；失败后追加 `提示：...`。

### 4. 可观测、评测与发布门

- `metrics` 从 journal 投影完成率、恢复率、结构回放、压缩回放、工具成功、
  审计完整、拒绝观测、token/成本、run 延迟与 TTFT。
- `gate-check.sh` 对缺失样本失败关闭；dashboard 增加门禁卡片。
- 真实任务 1h 门（产品口径 ≤1h）：DeepSeek `deepseek-flash`，30 轮 /
  240 任务，M1 **233/240 = 97.08%**，suite **23/30 = 76.66%**，completed
  replay **239/239 = 100%**，crash-free 100%，44m16s，约 ¥11.63。一次
  provider error 丢了一条 case 记录，历史 suite audit+replay 门为 29/30，
  CLI `passed=false`；任务级门通过，差异已写入证据。
- 8h 本地运行时 soak：Ollama `qwen3-8b-16k`，56 轮 / 448 任务，0 崩溃，
  completed replay 234/234，实际 8h26m51s。本地模型任务质量仅 21.2%，
  只证明运行时稳定性，不作为 M1 模型质量声明；8h DeepSeek 质量门不再声称。
- 100 题 AG-23 闭环：**96/100**；intro 20/20、追问 20/20、高风险 20/20、
  简单聊天 19/20、最新/本地 17/20；completed replay 78/78。
- 首 token 真实基线：n=20（10 短 + 10 长上下文）p50 1.50 s / p95 2.77 s；
  短 p50 1.24 / p95 1.94，长 p50 1.57 / p95 2.79；一个 provider，工具关闭。
- `release-check.sh` 在当前 tree 上执行 fmt、clippy、全量测试、HTTP e2e、
  claims 与 TUI PTY。`check-attribution.sh` 在本地 `main` 上仍因两条历史提交
  报红；公开发布必须从重建的干净历史切出。指标门因未传 `--metrics-dir`
  跳过。

### 5. 安全与运维

- 运行时 kill switch：`~/.1a/KILL` 存在时所有写工具失败关闭，只读工具
  保持可用。
- watchdog 升级阶梯与 checked `--execute pause|resume`；
- 0.97 新增 opt-in 自动遏制：`--watch --execute pause` 对每个 Pause/Repair
  pid 只发一次校验过的 SIGSTOP，Pause→Repair 不重复；不自动恢复、kill、
  重启或清锁。Unix only，仍有 cmdline 检查与信号之间的 TOCTOU。
- 威胁模型、审计包和独立代码级评审：0 个 P0，5 类 P1 已修（审批结果、
  子 Agent 权限、MCP 环境与注解、恢复重放、provider 重定向）；4 项 P1
  残余（browser_read SSRF、run journal 无哈希链、跨构建机可复现、历史署名）
  与 P2 清单见 `docs/audit-v0.97.md`。claims 233（209 ✅ / 16 ⚠️ / 8 ❌）；
  第三方审计/认证仍未完成。

## 验证摘要

| 项 | 结果 | 状态 |
|---|---|---|
| 全量测试（当前） | `cargo test --offline -j 2` 全绿 | local pass |
| fmt / clippy | clean | local pass |
| release-check | fmt、clippy、tests、HTTP e2e、verify.sh 通过；发布分支 attribution 干净；TUI PTY 检查在本机会话超时，`verify-claims --run` 未跑完 | partial |
| 2h 崩溃注入 soak | 28,552 cycles / 7,200.20s / exit 0 | local pass |
| 结构回放 | 100% on completed journals | local pass |
| 审计完整 | 100% under `AgentAnswer::validate` | local pass |
| 1h 真实任务门 | 30 轮 / 240 任务：M1 97.08%，suite 76.66%，M5 100%，44m16s，¥11.63 | local pass; one lost case record |
| 8h 本地模型 soak | 56 轮 / 448 任务，0 崩溃，replay 234/234，8h26m51s | runtime pass; local model quality 21.2%, not M1 |
| 100 题 AG-23 | 96/100，0 hard gate | local pass |
| M7 TTFT | n=20 p50 1.50 s / p95 2.77 s | one provider; tools disabled |
| M19–M23 | M19 85.00%, M20 5.13% (2/39), M21 2.50%, M22 0.00%, M23 null | **AI/bulk labels, not human blind review** |
| 独立代码级评审 | 0 P0；5 类 P1 已修；4 项 P1 残余 | not a third-party audit |
| 第三方审计 / macOS P-Gate | 未开始 | pending |

## 已知边界

- 0.97 不是生产稳定版；接口、配置和 journal 格式仍可能变化。
- 不能从 0.97 直接降级到 0.96.1 读取新 journal；升级前请备份。
- 旧二进制不能读含 `checkpoint_delta`/`model_request_delta` 的新 journal。
- 自动重启、外部 repair command、自动 resume 不在 0.97。
- watchdog 只支持 Unix 进程信号；日志无轮转。
- `cost_micros` 只有在 provider 配置了价格时才有意义；当前 DeepSeek 配置
  未嵌入价格。
- 人工盲评、第三方 A-Gate/W-Gate/安全认证、macOS P-Gate 仍未完成；M19–M23
  当前是 AI/批量标签，不能写成“人工盲评通过”。
- `docs/audit-v0.97.md` 仍有 4 项 P1 残余（browser_read SSRF、run journal
  哈希链、跨构建机可复现、历史署名），发布说明不得声称“已通过安全审计”。
- 8h DeepSeek 模型质量未声称；M1 质量声明限定在 ≤1h 任务口径。

## 升级与回滚

1. 备份 runtime home（`~/.1a`）和 journal 目录。
2. 用 0.97 二进制读取旧 journal 是支持的；反向不保证。
3. 回滚点与详细步骤见 `docs/rollback-v0.97.md`。
4. kill switch `~/.1a/KILL` 可阻止写工具；它不杀进程、不重启、不清理锁。

## 发布资产（本地候选）

- `1agent-0.97.0-linux-x86_64.tar.gz` — Linux x86_64 包；
- `SHA256SUMS` — 归档与 provenance 校验和；
- `linux-x86_64.json` — Linux 构建 provenance。

本地包已生成并验证：二进制 SHA-256
`36c6e44faa15e4011261f20b735738cd7fb05a9b9818ddd0340e0fad482ee3fb`，
`package.sh --linux-only` 两次构建 `reproducible=true`，内层/外层
`SHA256SUMS`、provenance、`--version`、init/doctor 与临时 prefix 安装均通过。
这是本地发布候选，不是公开发布公告；公开前仍需重建干净历史并确认资产。
