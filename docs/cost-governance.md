# 成本治理计划（0.97 产品卖点）

Status: **P0 implemented; P1/P2 pending**
Last updated: 2026-11-13 16:00 CST
Related: `docs/status-v0.97.md`, ADR-0126, ADR-0128

Implemented 2026-11-13: benchmark cost guard (`--max-cost-micros`,
`--checkpoint-every`, `--max-provider-error-bps`, `--min-crash-free-bps`),
benchmark preflight (`--estimate-only`), chat fast path (conservative
auto-detection), and the chat path's no-tools hard guard.
Per-turn TUI cost display was removed at the user's request; it is not part of
this release slice. Pending: prompt/cache/compression and the real-price
dashboard.

## 1. 为什么这是产品卖点

1Agent 的差异不是“能调模型”，而是“证据可追、成本可算”。用户不应该在月底看
provider 账单才知道一次提问花了多少钱。

0.97 把成本治理做成产品能力：

- 聊天不启动完整 Agent 循环；
- 每次运行都能看到 tokens 和估算成本；
- 长跑 benchmark 有预算、checkpoint 和熔断；
- 对外用 **cost per successful task** 说话，而不是只报 token 单价。

一句话：**Evidence-accountable, cost-accountable.**

## 2. 目标函数

不要只优化总 tokens。目标是：

```text
cost_per_successful_task = total_cost / successful_tasks
```

要同时看：

- cost per completed task
- calls per task
- tokens per call
- cache hit rate
- repair rate
- tool calls per task
- quality gate（M1/M5）不下降

## 3. 当前基线（2026-11-13）

`1agent cost` 统计到的 21 次运行：

| 指标 | 数值 |
|---|---:|
| 运行次数 | 21 |
| 模型调用 | 102 |
| 输入 tokens | 1,252,735 |
| 输出 tokens | 84,090 |
| 总 tokens | 1,336,825 |

典型任务：

- DeepTutor 安装：582,684 tokens，21 次调用
- 写入全局变量：111,822 tokens，9 次调用
- “启动指令是什么？”：99,661 tokens，8 次调用
- 自我介绍：4 次运行，11 次调用

系统提示词约 5–7k 字符（约 2–3k tokens），但平均每次调用输入约 1.2 万
tokens；差额来自工具清单、工具结果、会话上下文和多次循环。

320-run benchmark：2560 个任务执行，估计 13–33 元（以真实单价和
per-case journal 为准；当前 benchmark 没有保留 per-case journal）。它很可能
是 2026-11-13 账单的大头，必须单独核算。

2026-11-13 预检：核心套件 8 案 × 320 次，按参考单价 2000/3000 micros per k
计算，预估 10.3M 输入 + 2.0M 输出 tokens，约 **¥26.68**。这与当天的账单
量级一致；预检现在会在开跑前拦下这类长跑。

`configs/pi-deepseek.json` 目前没有真实单价，`1agent cost` 的 cost_micros 是
名义值，不是账单。实施成本治理的第一步是把真实输入/输出/缓存单价写进配置。

## 4. 五个杠杆

1. **调用次数**：8–21 次 → 1 次（Chat），或 1–2 次（DirectAnswer）。
2. **上下文**：静态提示词、工具 schema、工具结果、会话历史。
3. **输出**：完整证据 JSON vs 最小 JSON / 短结论。
4. **单价**：Chat 便宜模型、Work 强模型、缓存命中价。
5. **缓存**：静态前缀、工具结果、只读语义缓存。

## 5. 产品机制

### 5.1 Chat 快速路径

- 对短问候/简单问题做保守自动识别。
- 紧凑系统提示词（1–2k 字符）。
- `tools: []`，单次模型调用。
- 最小 JSON：`status` + `conclusion`，其余数组为空。
- 仍然写 journal，记录 usage，便于成本归属。

已实现：`looks_like_chat` 把问句、寒暄和自我介绍送进快速路径；带动作、
命令、代词、上下文指代或“继续”的请求仍走完整 Agent。检索、本地状态、
实时信息、项目/代码/文件类问题即使写成问句也强制完整 Agent。快速路径返回
`needs_evidence` 时自动升级一次完整 Agent。紧凑提示词包含模式与能力事实，
避免把“当前聊天轮”说成整个产品。自动识别只改变 TUI/CLI 的
`run_with_mode` 路径，benchmark 与 eval 继续走 `run` 的完整循环，避免污染
质量测量。当前没有单次 `max_tokens` 的 wire 字段，输出长度由最小 JSON
提示词约束；这是下一项。

目标：简单问题 **≤2k 输入 + ≤500 输出**。

### 5.2 DirectAnswer 默认不挂工具

- 只有明确动作（写文件、执行、检索、安装）才暴露工具。
- 简单问答、介绍、解释、寒暄不挂工具。
- 这是硬闸门，不依赖模型自觉。

已实现：聊天型 `DirectAnswer` 任务的 `tools` 为空，且步数锁定为 1。带
`帮我`、动作动词、`继续` 或附件的请求不进入快速路径，仍使用完整工具集。

### 5.3 提示词拆分与前缀缓存

- Chat / Work 两套系统提示词。
- 静态指令、工具 schema 放前缀；动态会话事实放后面的 user message。
- 保持可缓存前缀稳定，目标 cache hit 70–90%。
- 只在问题确实涉及提示词/作者时注入提示词库事实。

### 5.4 工具结果与会话压缩

- 工具结果只给模型 1–2k 摘要；完整内容留在 journal，按需再读。
- 会话上下文用结构化摘要，不每轮重发全文。
- 工具调用去重；重复读取走缓存。

### 5.5 模型路由

- Chat / 分类 / 摘要：便宜模型。
- Work / 证据 / 决策：强模型。
- 用现有 `ProviderManifest` 的 quality/cost/latency 路由；`/model` 面板拆成
  Chat 和 Work 两套 profile。
- 质量门禁不通过就不降级。

### 5.6 成本可见性

- 配置真实单价：`input_cost_micros_per_k` / `output_cost_micros_per_k`。
- TUI 每轮显示 tokens + 估算成本（本切片未做；用户要求不改信息流）。
- `1agent cost` 支持按模式、任务、成功任务分组（待做）。
- Session / run 预算；超过 `max_cost_micros` 以 `BudgetExhausted` 停止。
- 对外报告 cost per successful task。

### 5.7 Benchmark 成本闸门

- 开跑前预估：套件任务数 × runs × 每案 tokens × 单价；`--estimate-only`
  只打印预估值，`--max-cost-micros` 小于预估时直接拒绝启动。
- `--max-cost-micros`；运行中超过预算停止。
- checkpoint：20 / 40 / 80 / 160 / 320；completion 用 Wilson 95% 上界判断，
  小样本不会误停；crash-free 只有在剩余运行全不崩也救不回门槛时才停。
- Provider 熔断：401/429/超时率超过阈值立即停止整轮。
- 失败快停：整轮 completion 上界已经低于门槛时不再跑完。
- 不要缓存模型回答来省 benchmark 成本；那会污染质量结论。静态前缀缓存可以。

## 6. 分阶段计划

### P0（先做）

- [x] Benchmark 成本闸门：`--max-cost-micros`、`--checkpoint-every`、
  `--max-provider-error-bps`、`--min-crash-free-bps`；报告
  `stopped_early`/`stop_reason`/`cost_micros`/`provider_error_bps`。
- [x] Benchmark 开跑前成本预估：`--estimate-only`，超出
  `--max-cost-micros` 时拒绝启动。
- [x] Chat 快速路径（保守自动识别；不做显式命令，避免改信息流）。
- [x] DirectAnswer 聊天型任务默认不挂工具，步数锁定为 1。
- [x] 真实单价写入 `configs/pi-deepseek.json`（参考值，按账单更新）。
- [ ] TUI 每轮 tokens/成本显示（本切片未做）。

### P1

- Chat / Work 提示词拆分；静态前缀缓存。
- 工具检索；工具结果压缩；会话摘要。
- `max_tokens` 按模式；最小 JSON；repair 最多一次。

### P2

- Chat / Work 两套模型 profile；便宜模型做分类/摘要。
- Tool result cache；只读语义缓存；cache hit 看板。
- cost per successful task 报表与预算告警。

## 7. 预期收益（基于 2026-11-13 数据）

| 场景 | 现在 | 目标 | 预期降幅 |
|---|---|---|---:|
| 简单聊天/问答 | 8–21 次调用，1–10 万 tokens | 1 次，2–3k tokens | **5–20 倍** |
| 工作型任务 | 20–60 万 tokens，9–21 次调用 | 少 30–50% | **1.5–2 倍** |
| 失败长跑 benchmark | 320 次跑满 | checkpoint + 熔断 | **50–80%** |
| 综合 | — | — | **约 40–70%** |

这些是 token/调用级估算；实际 RMB 取决于 provider 真实单价、缓存价和
benchmark per-case 用量。对外不承诺未测量的具体金额。

## 8. 验收标准

- 固定 20 题聊天集：单题成本降 ≥5 倍，质量不下降；记录 baseline 和 after。
- 固定工作任务集：tokens 降 30–50%，证据/审计不回归。
- 20-run benchmark 样本：单次运行成本降 ≥30%，gate 结果不下降。
- 失败 benchmark：checkpoint 能在 80–160 次内停止，节省 ≥50%。
- `1agent cost` 与 provider 控制台账单可对账（同一时间窗、同一模型）。
- Chat key/内容不进 transcript/journal；成本记录不含 secret。
- M1/M5 仍是发布门禁；成本治理不能用来掩盖质量失败。

## 9. 风险与边界

- 便宜模型可能降低质量；必须过 golden set / 小样本回归。
- 缓存可能带来 stale 答案；证据型任务不缓存回答。
- Provider 的 `/models`、缓存价、`reasoning_effort` 支持情况不同；失败要可见。
- Benchmark 缓存答案会破坏测量；只允许静态前缀/工具结果缓存。
- 成本归属依赖真实单价和保留 per-case journal；当前 benchmark 还没有保留。
- 不把“省 token”当作唯一目标；最终看 cost per successful task。

## 10. 对外表述

- “聊天不会唤醒整个 Agent。”
- “每次运行都能看到 tokens 和估算成本。”
- “长跑有预算、checkpoint 和熔断，不会跑完才发现失败。”
- “我们报 cost per successful task，不报好看的 token 单价。”
- “质量门禁不通过，成本再低也不算成功。”
