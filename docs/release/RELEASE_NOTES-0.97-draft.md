# Release Notes — 0.97.0 Reliable Loop（候选稿）

[简体中文](RELEASE_NOTES-0.97-draft.md) · [English](RELEASE_NOTES-0.97-draft.en.md)

Date: 2026-11-13

Status: **release-candidate draft, not published; 8h gate failed**

Pending: reliability fixes and a new 8h run, broader TTFT sample, human
blind-review labels, external A-Gate/W-Gate/security review, macOS P-Gate,
release package.

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
- `/clarify` 全屏面板、错误恢复提示、transcript 投影缓存（200 条长
  transcript release 帧 p95 1.26 ms）。
- 行模式与交互模式共用同一错误 hint；失败后追加 `提示：...`。

### 4. 可观测、评测与发布门

- `metrics` 从 journal 投影完成率、恢复率、结构回放、压缩回放、工具成功、
  审计完整、拒绝观测、token/成本、run 延迟与 TTFT。
- `gate-check.sh` 对缺失样本失败关闭；dashboard 增加门禁卡片。
- 真实任务套件 v1.1：DeepSeek `deepseek-flash` 两次 16/16；早期 v1.0
  基线 17/24（70.83%）。8h 重复基准已完成但**未达 0.97 目标**：136/280
  全任务通过（48.57%，0.97 目标 ≥50%，差 1.43 个百分点）、238/280
  suite 回放（85%，不是 M5 的已完成 run 分母）、280/280 crash-free。
- 首 token 真实基线：3 次短流式任务 p50 1.08 s / p95 2.48 s；扩展样本
  17/20（p50 1.31 s / p95 2.26 s），3 个因首次响应是工具调用而缺少
  content token，需要重跑。
- `release-check.sh` 在 clean tree `39ea96e`（由 `509061c` 去除工具署名后
  重写）上 exit 0：fmt、clippy、
  1261 passed / 0 failed / 13 ignored、HTTP e2e、CI status 0、claims 证据
  （新增 3 个 attribution 回归测试后当前全量为 1264）
  执行、TUI PTY 交互。指标门因未传 `--metrics-dir` 跳过。

### 5. 安全与运维

- 运行时 kill switch：`~/.1a/KILL` 存在时所有写工具失败关闭，只读工具
  保持可用。
- watchdog 升级阶梯与 checked `--execute pause|resume`；
- 0.97 新增 opt-in 自动遏制：`--watch --execute pause` 对每个 Pause/Repair
  pid 只发一次校验过的 SIGSTOP，Pause→Repair 不重复；不自动恢复、kill、
  重启或清锁。Unix only，仍有 cmdline 检查与信号之间的 TOCTOU。
- 威胁模型、审计包草稿、claims 200 ✅ / 16 ⚠️ / 8 ❌；外部审计未开始。

## 验证摘要

| 项 | 结果 | 状态 |
|---|---|---|
| 全量测试（当前） | 1264 passed / 0 failed / 13 ignored | local pass |
| fmt / clippy | clean | local pass |
| release-check | exit 0（指标门跳过） | local pass |
| 2h 崩溃注入 soak | 28,552 cycles / 7,200.20s / exit 0 | local pass |
| 结构回放 | 100% on completed journals | local pass |
| 审计完整 | 100% under `AgentAnswer::validate` | local pass |
| 真实任务 v1.1 | 16/16（两次短样本） | local pass, not 8h |
| 8h M1/M2/M3 | 136/280 completed (48.57%; 0.97 target ≥50%), 238/280 suite replay (85%; M5 needs completed-run calc), 280/280 crash-free | **target missed** |
| M7 TTFT | n=3 p50 1.08 s / p95 2.48 s; n=17 partial p50 1.31 s / p95 2.26 s | small/partial sample |
| M19–M23 盲评 | packet pipeline ready | human labels pending |
| 外部审计 / macOS P-Gate | 未开始 | pending |

## 已知边界

- 0.97 不是生产稳定版；接口、配置和 journal 格式仍可能变化。
- 不能从 0.97 直接降级到 0.96.1 读取新 journal；升级前请备份。
- 旧二进制不能读含 `checkpoint_delta`/`model_request_delta` 的新 journal。
- 自动重启、外部 repair command、自动 resume 不在 0.97。
- watchdog 只支持 Unix 进程信号；日志无轮转。
- `cost_micros` 只有在 provider 配置了价格时才有意义；当前 DeepSeek 配置
  未嵌入价格。
- M1/M2/M3、M7 扩展样本、M19–M23 人工标签、外部 A-Gate/W-Gate/安全审计、
  macOS P-Gate 仍未完成；不能据此宣称 Super-Agent 或生产稳定。

## 升级与回滚

1. 备份 runtime home（`~/.1a`）和 journal 目录。
2. 用 0.97 二进制读取旧 journal 是支持的；反向不保证。
3. 回滚点与详细步骤见 `docs/rollback-v0.97.md`。
4. kill switch `~/.1a/KILL` 可阻止写工具；它不杀进程、不重启、不清理锁。

## 发布资产（待生成）

- `1agent-0.97.0-linux-x86_64.tar.gz` — Linux x86_64 包；
- `SHA256SUMS` — 归档与 provenance 校验和；
- `linux-x86_64.json` — Linux 构建 provenance。

资产尚未生成；本文件不是发布公告。
