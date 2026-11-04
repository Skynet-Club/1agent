# NeedGraph 契约（提案）

Date: 2026-11-04
Status: proposed; additive to protocol v1; no breaking change

## 1. 目的

用形式化结构承载"请求 → 手段 → 目的"的回溯，使回溯可校验、可审计、可回放。

对应既有学术结构：

- i\* (Yu 1995)：`Task --means-end--> Goal`，`Goal --contribution--> Softgoal`
- KAOS (van Lamsweerde)：AND/OR goal refinement tree
- Toulmin (1958)：claim / data / warrant / qualifier / rebuttal
- GSN (ACWG v3)：claim → strategy → sub-claim → evidence

现有 `domain.rs` 的 `Claim { status, sources, scope }` 与 `FlipCondition` 已经是
Toulmin 形状，因此本契约是**在其之上加一层**，不是替换。

## 2. 结构

```text
NeedGraph
  stated_request: String            // 逐字，不可变，永不被覆盖
  classification: End | Means       // 默认 End
  classification_evidence: Vec<Evidence>
  hops: Vec<Hop>                    // 0..=3，默认 1
  chosen_level: Option<HopId>       // 用户裁决的结果
  decision_gate: DecisionGate

Hop
  id
  from: Means                       // 当前层的手段
  to: Need                          // 上一层的目的
  why: String                       // 为什么 from 服务于 to
  stop_rule: { verifiable, deliverable, better_substitute_exists }   // 三条件
  domain_assumptions: Vec<{ statement, falsifier }>   // 必填，可为空数组但需显式
  rejected_alternative: String      // 为什么不直接用上一层手段
  falsifier: String                 // 什么情况下这一跳是错的
  counter_argument: String          // 原方案仍然成立的理由

Substitute
  hop_id
  proposal: String
  preconditions: Vec<String>        // 用原始 constraints 复核
  new_costs: Vec<String>            // 必填非空
  expected_effect: String
  causal_check: { dag_edges, separations }   // 见 §4
```

## 3. 机器可检不变量

1. `stated_request` 在整条链生命周期内字节不变。
2. `hops` 无环；`hops.len() <= 3`。
3. `classification == End` 时 `hops` 为空且不产生 `Substitute`。
4. 每个 Hop 的 `falsifier`、`counter_argument` 非空。
5. 每个 Substitute 的 `new_costs` 非空。
6. `chosen_level` 只能由 `DecisionGate::Confirmed` 产生。
7. 未确认时不得产生任何 `action_receipt`（与现有 `AgentAnswer::validate` 一致）。
8. 3 跳以上的图直接拒绝，不进入校验。

上述 1–8 全部可用性质测试（proptest）覆盖，无需网络与模型。

## 4. 因果检验

对每个 Substitute，构造 DAG：

```text
original_means -> need        // 原路径
substitute     -> need        // 替代路径
confounders...                // 来自 domain_assumptions
```

判定：若 `need ⊥ original_means | substitute` 在 d-separation 意义下成立，
则替代方案确实切断了"必须使用原手段"的依赖；否则该替代只是看起来像。

实现路径（择一）：

- 外部：dagitty（web/R）或 DoWhy（Python），用于研究阶段人工核验；
- 内部：Rust 内实现小型 DAG + d-separation（约 150 行），用于回归测试。

第一阶段先做外部核验 + 固定夹具，不急于内嵌。

## 5. 与现有契约的组合方式

加法式，不破坏 v1/v2：

- `TaskRequest` 不改。NeedGraph 是 run 开始后的**预阶段产物**，写入 journal。
- `AgentAnswer` 不改。选定层级后，仍走原有 answer 契约求解。
- 新增 journal 事件：`need_graph_proposed`、`need_graph_confirmed`、`need_graph_rejected`。
- 新增可选 CLI：`run --regress <policy>`，默认关闭。
- `infer_mode` 不改；新增独立的 `Regress` 预模式，在 `infer_mode` 之前运行。

## 6. 待定问题

1. 若用户拒绝全部重构，是否仍记录完整的被否决链（用于后续同类请求）？
2. 多利益相关者时，`chosen_level` 属于谁？（当前假设单一决策者）
3. `domain_assumptions` 与现有 `AssumptionItem` 是否合并？两者语义不同：
   前者是世界事实，后者是推理前提。
4. 跨 run 的经验复用是否适用于 NeedGraph（L2 经验存储）？若适用，需新的证据类别。
