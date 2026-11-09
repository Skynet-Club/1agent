# ADR 0019: 回答之前先判断请求是否为手段

- Status: proposed
- Date: 2026-11-04

## Context

1A 目前的认知模式为 `DirectAnswer / EvidenceResearch / DecisionAnalysis / Execute /
Diagnosis / Escalate`，`infer_mode()` 对给定 objective 分类，从不质疑 objective
本身。核验结果：

- 75 条真实 run 中，模式分布 54/18/3，停止原因 `needs_evidence` 44 次；
  系统会因"事实不足"停下，从不因"目标不足"停下；
- 94 份 status 文档、10 份 audit、18 个 ADR 中，涉及用户需求或意图校验的：0 份；
- 仓库与设计笔记中，需求回溯类词汇命中数：0。

即：产品的核心命题存在于口述，未进入任何可执行契约，因而不可测、不可审计、
也无法阻止实现漂移。

## Decision

1. 引入**预阶段**：在 `infer_mode` 之前运行 `Regress`，产出 `NeedGraph`。
2. **默认不重构**：先做 means/end 分类，默认判为 end。
3. 重构只在停止规则三条件（可验证 ∧ 可交付 ∧ 存在更优替代）同时成立时继续。
4. 原请求逐字不可变；重构是提议 + 决策门，永不代替用户决定。
5. 该阶段为**加法式**：不改 `TaskRequest` / `AgentAnswer` / `infer_mode`，
   仅新增 journal 事件与可选 CLI 开关（默认关闭）。
6. 在评测协议预注册并冻结之前，不写实现代码。

## Rejected alternatives

- **直接扩展 `CognitiveMode`**：会让"质疑目标"变成一种可选风格，而不是默认前置
  关卡；且破坏既有模式推断契约。
- **改 `TaskRequest` 增加 need 字段**：破坏 v1 契约，且把不可观测的语义写进输入。
- **总是回溯**：绝大多数请求是字面请求，误重构的代价高于不重构。
- **用"需求理解得更深"作为正确性证据**：不可验证。只接受行为证据（接受/拒绝）。
- **先写实现再评测**：会让门槛事后可调，等于没有门槛。

## Consequences

- 新增 `docs/thesis/` 三份文档与一份预注册协议；在协议冻结前实现被阻断。
- 需要新增 `proptest` 作为 dev-dependency，用于不变量证明。
- 需要外部标注者（≥2，非设计者）与红队 agent，成本在人力不在算力。
- 若杀项条件触发，本 ADR 结论应被推翻并记录，而不是静默调参。
- 与 MCP/A2A 的关系被重新定位：工具清单即"可交付替代手段"的库存，
  因此 MCP 属于本机制的基础设施，而非后期互操作加分项。
