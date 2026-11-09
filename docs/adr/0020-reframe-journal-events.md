# ADR 0020: 用 journal 承载需求重构的提议与决定

- Status: accepted for the journal layer; CLI still unimplemented
- Date: 2026-11-04

## Context

`ReframeProposal` 及其不变式已落地（`src/reframe.rs`，提交 `fee3daa`）。
它目前只存在于库中：没有命令产生它，没有地方保存它。

产品的核心承诺是"三个月后还能查这张收据"。要实现它，提议与决定必须进入
journal——项目的第一事实源——而不是另开一个存储。

同时，`src/reframe.rs` 的注释已经写明：类型是"可审计的那一半"，
模型行为留在研究脚本里。现在要补的是"记录的那一半"。

## Decision

新增两个 journal 事件，加法式加入现有 `JournalEventKind`：

```text
reframe_proposed
  proposal: ReframeProposal        // 必须通过 validate()
  provider: { provider_id, model }
  prompt_sha256: String

reframe_decided
  choice: accepted { option_index, substitute_index? }
        | rejected { reason: RejectedReason }
        | deferred
  actor: String                    // 只能是人；模型不得自我确认
  decided_at_unix_ms: u128
```

`RejectedReason` 取值固定为五类，与评测协议 §4 的标注类别一致：

```text
cost_transferred     新增代价被转嫁给用户
constraint_unmet     原始约束未被满足
prefer_literal       我就是要原方案
unverifiable         提出的目的无法验证
other
```

### 不变量（机检）

1. `reframe_proposed.proposal.validate()` 必须通过，否则拒绝写入。
2. `actor` 不得为空，且不得等于产生该提议的 provider 标识。
3. `accepted.option_index` 必须落在 `options` 范围内。
4. 同一次 run 内，一个 `reframe_proposed` 最多对应一个 `reframe_decided`。
5. `reframe_decided` 必须能在此前找到同一 run 的 `reframe_proposed`。
6. `stated_request` 在整个 run 生命周期内字节不变。

### 重放与审计

- `replay_events` 重建"最后一次提议 + 其决定"，供 `/receipt` 使用。
- `audit_events` 增加三个发现码：`reframe_decision_without_proposal`、
  `reframe_duplicate_decision`、`reframe_option_out_of_range`。
- 未决定的提议是合法状态（用户还没答），不得视为错误。

### 一次决定 = 一次 run

实现前核对 audit 规则时发现：`MissingAnswerForStop` 会对 `Completed` /
`NeedsEvidence` / `NeedsAuthorization` 触发，而 `Journal::create` 没有
"重新打开并追加"的路径。因此"先提议、三天后再追加决定"做不到，硬做会让
audit 报假阳性。

正确形状是：**一次决定就是一个完整 run**。

```text
RunStarted(DirectAnswer)
ReframeProposed
ReframeDecided
AnswerProduced      // 结论即"已采纳第 2 项"或"拒绝：我就是要原方案"
RunStopped(Completed)
```

这样不需要新的 `StopReason`，不需要跨文件拼接，也不需要重开 journal。
`AnswerProduced` 不是伪造：它就是这次 run 的真实产出。

### 协议版本

`PROTOCOL_VERSION` **保持 1**。原计划升到 2，但实际核对后发现：
`experience.rs` 也做版本相等检查，升版本会连带影响历史 journal 与
经验存储的兼容路径，而新增事件本身对旧二进制已经是 fail-closed
（未知变体直接反序列化失败）。在收益不明确时不动版本号。

## Rejected alternatives

- **另开 `reframes/` 存储**：制造第二个事实源，违反"journal 是第一事实源"。
  一旦两处不一致，就无法回答"当时到底记了什么"。
- **复用 `AnswerProduced`**：把重构塞进答案会让"结论"与"提议"语义混淆，
  且收据会依赖答案 schema，等于把两件事绑死。
- **只写 stdout 不入库**：收据变成一次性输出，三个月后无从查起。
- **让模型写 `reframe_decided`**：决定必须由人做出。模型自确认是本项目
  在 `product-boundary.md` 里明确禁止的行为。

## Consequences

- `/receipt` 可以完全由 journal 重建，不需要额外状态。
- `obviation_rate` 与 `regret_rate` 第一次有了数据来源：
  `accepted` 计数即 obviation 的分子，`rejected{prefer_literal}` 即 regret。
- journal 事件枚举扩大，`audit` 与 `replay` 的匹配必须同步更新，
  否则编译期即失败（这是好事）。
- 一次决定结束一次 run，因此没有"挂起的提议"。用户没答就是没跑，
  不会留下半开状态。
- 旧 journal 保持可读；新 journal 需要新的二进制（未知事件变体 fail closed）。
