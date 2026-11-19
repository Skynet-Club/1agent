# 1Agent / Super-Agent 本体方案（1AO）

Status: **P0 只读投影已实现；P1 决策/动作层设计提案（2026-11 Palantir 校准）**
Date: 2026-11-18
Revision: industry/academic review incorporated (Palantir, Fabric, Netflix, LinkedIn, Uber, Atlas, W3C, GraphRAG, bitemporal)
Scope: 0.98 现有会话、运行、证据、分队与治理数据 → Super-Agent 的可查询语义层

## 0. 结论先行

我建议采用**运营型本体类型系统 + 轻量语义投影**，而不是只做一张 RDF 词表，也不是立刻上重型 OWL：

```text
journal / session.json / subagent registry        （事实层，唯一审计源）
        │
        ├── Type Registry                            （对象/关系/动作/函数/接口/值类型）
        ├── Data Binding                             （类型/属性 ↔ journal/session/registry 字段）
        ├── JSON-LD 1.1 + PROV-O                     （交换层，稳定 IRI）
        ├── SHACL + JSON Schema                      （操作校验层）
        └── SQLite / GraphQL / 报表投影               （查询/UI 层，按需）
```

- **OWL 2 DL + 推理机**暂不进入运行时；最多作为外部交换/研究工具。
- 第一阶段只做**只读导出**：不改变模型 prompt、不改变 journal 写入、不改变任何运行行为。
- 本体不只是“语义层”：按 Palantir/Fabric 的公开做法，至少要包含 **对象类型、关系类型、动作类型、数据绑定和安全策略**；否则只是一个漂亮的 ER 图。
- 先回答 16 个 competency questions，再决定要不要加图数据库或推理机。
- 本体的价值是**语义、溯源、跨会话/跨 Agent 连接**；它不会让模型结论变真，也不会替代 JSON Schema。
- **journal 仍是事实源**；本体是可重建的投影。导出失败不能影响运行。
- 2026-11-16：P0 已落地为 `src/ontology/` 与 `1agent ontology`；只读、
  确定性 JSON-LD、内部校验。动作/安全全链路与 SquadMember 父链仍是 P1。

## 0.1 国际实践与论文校准（2026-11 补充）

以下是公开资料中与我们最相关的做法，以及它们对 1AO 的修正。

| 来源 | 核心做法 | 对 1AO 的修正 |
|---|---|---|
| **Palantir Foundry Ontology** | 官方文档把它描述为 operational layer：object types、properties、link types、action types、functions、dynamic security；“Ontology is not a semantic layer” 的表述强调它同时承载数据、逻辑、动作与安全 | 补 `ActionType/ActionInvocation/PolicyDecision/AccessPolicy`；不要把本体写成只读 ER 图 |
| **Microsoft Fabric Ontology** | entity type、entity key、property（静态/时序）、relationship type、data binding、metrics；类型可以没有绑定，绑定把类型映射到 OneLake/Eventhouse 数据 | 补 `DataBinding`、key mapping、metric；导出前必须有显式字段映射 |
| **Netflix UDA / Upper** | RDF + SHACL 的 upper metamodel；self-describing、closed for modification/open for extension；named graph；把同一模型投影到 GraphQL、Avro、Iceberg、SQL | 采用“model once, represent everywhere”：类型注册表生成 JSON-LD/SQL/GraphQL，而不是手写多套模型 |
| **LinkedIn Economic Graph** | 自研属性图；稳定 URN 命名空间；typed edges；实体属性带 confidence；实体解析/规范化是产品能力 | 补稳定 ID、alias/merge 溯源、confidence；不要用 PID/标题当身份 |
| **Uber Databook / ontology** | 元数据图 + 端到端 lineage + controlled vocabulary；公开材料提到 OWL-Time、schema.org 对齐、table/field 级隐私标注 | 把技术元数据/运行元数据与业务本体分层；补 bitemporal、PII/sensitivity |
| **Apache Atlas / Purview** | type system：entityDefs、relationshipDefs、classificationDefs、enumDefs、structDefs；technical + business metadata；lineage | 用“类型定义 + 分类 + 关系定义”的注册表，而不是只写 OWL class |
| **Google Knowledge Vault** | 固定 ontology 上的概率知识融合；抽取器置信度 + 图先验；provenance/confidence；局部封闭世界假设 | 结论/证据要带 confidence 与融合来源；不要把“有记录”当“为真” |
| **FIBO** | 模块化 OWL、行业联盟治理、operational ontology、可推理的金融概念 | 可复用上层/行业词表，但不要一开始做全行业公理 |
| **W3C PROV-O / SHACL / OWL 2** | PROV 做溯源；SHACL 做封闭世界校验；OWL 做概念/推理；三者分层 | 我们的分层方向正确；补 profile/版本/发布兼容性 |
| **Bitemporal KG**（Chekol & Stuckenschmidt, TIME 2019） | 事实同时带 valid time 与 transaction time；支持历史重建与更正 | 补 `validFrom/validTo/recordedAt`；区分“事实何时为真”和“我们何时记录” |
| **OntoClean**（Guarino & Welty, 2002） | 用 identity、unity、rigidity 检查 subsumption；防止把角色/阶段误当刚性类 | 对 `Agent/Run/Claim/Artifact` 做身份与刚性审查；不要把 `Paused` 建成子类 |
| **Commitment/ACL 研究**（Boella 等；FIPA ACL） | 多 Agent 通信不仅是消息，还有承诺、接受、拒绝、责任 | 补 `Commitment/Delegation/Acceptance/Refusal`，用于 Super-Agent 任务委派 |
| **Microsoft GraphRAG**（Edge et al., 2024） | LLM 抽实体/关系 + 社区摘要，适合全局语料问答 | 可作为“非结构化证据检索”补充；不能替代 journal 事实与 provenance |
| **LLM ontology engineering 综述（2025）** | LLM 可辅助抽取/建模，但评估碎片化、缺标准、需 human-in-the-loop | LLM 只做候选映射/草稿；人工评审 + CQ/SHACL 测试后才能入库 |
| **A2A / Agent Card** | Agent Card、Task 状态、Message/Part、Artifact、contextId；跨组织互操作 | Super-Agent 的对外任务/产物可映射 A2A；内部仍用 journal |
| **OpenTelemetry GenAI semconv** | `invoke_agent`、`execute_tool`、conversation/agent/tool 属性；trace 树 | 导出 OTel span 作为运行时可观测层；与 PROV/本体互链，不混为事实 |

### 0.2 由此产生的更优建议

1. **1AO 的第一等产物不是 `.ttl`，而是 Type Registry**：`ObjectType`、`PropertyType`、`LinkType`、`ActionType`、`FunctionType`、`InterfaceType`、`ValueType`。JSON-LD/OWL/SHACL 都从它生成或校验。
2. **必须有 Data Binding**：每个对象属性映射到 journal 事件/字段、session.json 字段或 registry 字段；没有绑定的类型只是文档。
3. **必须有 Action/Security 层**：`ActionType`（可逆性、风险、所需权限）、`ActionInvocation`、`PolicyDecision`、`Approval`、`AccessPolicy`、`Purpose`。这直接对应 1Agent 的 `ActionCard`、`AuthorityProfile`、`ExecutionGate`。
4. **身份与实体解析**：稳定 IRI + `aliasOf`/`mergedInto`/`sameAs` + 合并 provenance；artifact 以 SHA-256 为主身份；PID 不是身份。
5. **Bitemporal**：每个事实至少区分 `observedAt`（世界/证据时间）、`recordedAt`（journal 写入时间）、`validFrom/validTo`（有效期）；支持“当时知道什么”和“现在知道什么”。
6. **Commitment/Delegation**：Super-Agent 不只是 spawn 子进程，还有任务分配、接受/拒绝、完成承诺、取消责任；这些要成为一等实体。
7. **互操作投影**：JSON-LD/PROV 用于外部；OTel span 用于可观测；A2A AgentCard/Task/Artifact 用于跨组织；内部 SQLite/GraphQL 用于 UI/查询。模型只定义一次。
8. **评估必须量化**：CQ 覆盖率、绑定覆盖率、实体解析 precision/recall、SHACL 通过率、导出确定性、查询延迟、人工复核一致率；LLM 自动评估不能单独作数。

### 0.3 Palantir 2026 运营层校准（决策、动作、场景、权限）

2026 年重新阅读 Palantir 官方 Ontology/AIP/OSDK/OMCP 文档后，1AO 的重心需要再修正一次：Palantir 的价值不在“对象图”，而在**把决策作为运营单位**。

官方定义：Ontology 是组织的 operational layer，是组织的 digital twin；每个运营决策由四部分组成：

| 组成 | Palantir 的含义 | 1AO 对应 |
|---|---|---|
| **Data** | 企业数据 + “decision data”（决策上下文、评估过的选项、下游影响） | journal、evidence、memory、context |
| **Logic** | 确定性函数、模型、优化器；与非确定性 LLM 推理互补 | 工具、能力 manifest、确定性计算 |
| **Action** | ActionType 是受权限约束的事务：参数、规则、submission criteria、副作用 | `ActionCard`、`ExecutionGate`、tool call |
| **Security** | 对象/属性/单元格级策略，运行时动态计算；工具访问依赖对象访问 | `AuthorityProfile`、`PolicyDecision`、`PermissionSet` |

由此产生六个必须吸收的设计点：

1. **`Decision` 必须是一等实体**，不是 Answer 的附属。至少包含：问题、上下文、选项、标准、证据、选择、动作、审批、结果、修正、lineage。
2. **`ActionType` 必须是类型注册表的一部分**，不是临时 tool call。字段：参数 JSON Schema、前置条件/submission criteria、风险、可逆性、审批策略、副作用、后置条件、幂等键、版本。
3. **读写工具不对称**：读操作可以收敛为少量查询工具；写操作应一个 ActionType 一个工具，模型从“预定义业务动作”中选择，而不是自由组合底层写操作。
4. **Scenario/Proposal 是必需层**：先在沙箱里 staged changes，人类看 diff/影响，再通过受权限约束的 Action 原子合并；不要“先写生产再审批”。
5. **决策血缘**：记录“何时、基于哪版数据、通过哪个应用/agent、谁批准、结果如何”；这正是 1AO 的 `DecisionLineage`，比单纯 tool trace 更有产品价值。
6. **本体驱动能力**：ObjectType / QueryFunction / ActionType 应生成 `CapabilityManifest`、ToolSpec、MCP tool；这样新增业务动作不需要同时改工具、权限、CLI、MCP 四处。

同时要避免三个误区：

- **不要把 Palantir 当 W3C 本体**：它是 platform-native，官方文档不使用 OWL/RDF/SPARQL/SHACL；有分析指出它缺少 subsumption/inference，甚至把 ontology 当作 1:1 的权限/租户空间。我们保留 JSON-LD/PROV/SHACL 的可移植性，但不做 OWL 推理机。
- **不要只做只读导出**：没有 Action/Security/Decision 的本体只是“数字盆景”。1AO P0 的只读是安全基线，但 P1 必须进入动作与决策层。
- **不要克隆完整平台**：先选一个垂直切片（文档评审发布或代码变更发布），做 3–5 个 ActionType，用 CQ 和验收证明价值，再扩展。

**对 1AO 路线的影响**：原 P1 “补 lineage、动作与安全”升级为 **P1 决策/动作层**，优先顺序改为 `Decision/Option/Criterion/ActionType/ActionInvocation/Outcome` → typed tools → Scenario → object/action policy → Ontology MCP。详细里程碑见 `docs/1.1-standard-harness-plan.md` §11。

## 1. 本体是什么、不是什么

### 1.1 是什么

- 对共享概念化的显式、形式化规范（Gruber）。
- 定义类、属性、关系、个体、约束，让机器可以一致地解释：
  - 这次运行的结论依赖哪些证据？
  - 证据由哪个工具、哪个队员、哪次运行产生？
  - 谁授权了不可逆动作？
- 在 1Agent 里，它首先是一个**语义投影**，不是新的主存储。

### 1.2 不是什么

- 不是 JSON Schema 的替代品。JSON Schema 负责消息形状；本体负责概念和关系。
- 不是数据库。可以投影到 SQLite/图库，但不要求。
- 不是“让模型更聪明”的 prompt。
- 不是把自然语言结论变成事实的机器。
- 不存密钥、原始 prompt、完整文件内容；这些只保留引用或 SHA-256。

### 1.3 开放世界与封闭世界

- OWL 默认开放世界：没有声明不等于假。适合概念推理，不适合“缺字段就拒绝”。
- SHACL/JSON Schema 是封闭世界校验：适合运行前/导出前检查。
- 因此本方案用 **OWL/JSON-LD 表达语义，SHACL 做操作约束**，两者不混用。

## 2. 目标与非目标

### 2.1 目标

1. 让每个用户可见结论都能沿 `Claim → Evidence → Observation → ToolInvocation → Run → Agent` 回溯。
2. 让会话、运行、队员、父任务、审批、artifact 之间有关系，而不是靠目录和文件名猜。
3. 让 0.98 已有 journal/session/registry 数据可以被第三方以 JSON-LD/PROV 兼容形式消费。
4. 为 Super-Agent 的跨会话、跨队员、持久任务提供稳定身份和关系。
5. 保持现有 `journal` 的审计地位不变；本体导出失败不影响运行。

### 2.2 非目标（v1 明确不做）

- 不在运行时启动 OWL reasoner。
- 不把 journal 迁移到三元组数据库。
- 不要求模型输出新的必填字段。
- 不建模完整 BFO/DOLCE/SUMO 上层本体。
- 不导出模型思维链、原始工具全文、密钥或完整文件内容。
- 不把 PID 当持久身份；PID 只是进程观测值。

## 3. Competency questions（验收问题）

本体是否成功，先看这些问题能否用导出数据回答：

| # | 问题 | 主要数据 |
|---|---|---|
| CQ1 | 一个 `completed` 结论里，哪些 claim 没有 evidence？ | Answer、Claim、Evidence |
| CQ2 | claim X 由哪些 evidence 支持？evidence 来自哪个 tool observation？ | Claim、Evidence、Observation |
| CQ3 | artifact A 由哪次 run、哪个队员、哪个 tool call 产生？SHA 是多少？ | Artifact、Run、SquadMember、ToolInvocation |
| CQ4 | 某个队员由哪个父 run、父会话、哪个 `spawn_agent` 调用启动？ | SquadMember、Run、Session、ToolInvocation |
| CQ5 | 某次工具执行由哪个 authority profile 授权？是否有审批？ | ToolInvocation、PolicyDecision、Approval |
| CQ6 | 哪些任务因为 `needs_authorization` 阻塞？阻塞在哪个 actor/scope？ | Run、Task、PolicyDecision |
| CQ7 | 哪些答案是 conditional？翻转条件指向哪些 claim/unknown？ | Answer、FlipCondition、Claim、Unknown |
| CQ8 | 哪些 evidence 已过期（`expires_unix_ms`）或观测时间不明？ | Evidence |
| CQ9 | 哪些 run 从哪个 checkpoint 恢复？是否重复副作用？ | Run、Checkpoint、ResumeStarted |
| CQ10 | 哪个会话从哪个父会话 fork 出来？ | Session、forkedFrom |
| CQ11 | 评测 case/run 与本体的 claim/run 如何对应？ | EvalCase、EvalRun、Run |
| CQ12 | 哪个 artifact 在审批后 SHA 发生变化？ | Artifact、Approval、SHA |
| CQ13 | 哪些队员失败/超时？父会话最后怎么处理？ | SquadMember、Run、Answer |
| CQ14 | 哪个 provider/model/route 产生了某条 claim？成本多少？ | Run、RouteSelected、Claim、Usage |
| CQ15 | 哪些用户可见结论没有 audit/replay 通过的 journal？ | Answer、Journal、Audit |
| CQ16 | 哪些不可逆动作被谁批准、何时消费？ | ActionCard、Approval、ActionLog |

### 3.1 决策/动作 CQ（P0.1）

| # | 问题 | 数据 |
|---|---|---|
| CQ17 | 这次决策考虑过哪些选项，选中/拒绝了哪个？ | Decision、Option、ReframeDecided |
| CQ18 | 决策用了哪些判断标准，是否满足？ | Criterion、ReframeProposed.stop_rule |
| CQ19 | 决策关联了哪些 ActionInvocation，策略结果是什么？ | Decision.hasAction、ActionInvocation、PolicyDecision |
| CQ20 | 决策的 Outcome 和产物是什么？ | Outcome、Artifact |
| CQ21 | 哪些决策缺少 evidence/approval，不能当作已完成？ | Decision、Claim、Evidence、Approval |
| CQ22 | 某个 ActionType 的参数、风险、可逆性和审批要求是什么？ | Action Registry（P1） |

P0.1 的 golden tests 必须至少覆盖 CQ17–CQ20；CQ21 是负例门禁；CQ22 在 Action Registry 落地后启用。

## 4. 命名空间与身份

### 4.1 命名空间

暂定：

```text
one:  https://1agent.dev/ontology/0.98#
prov: http://www.w3.org/ns/prov#
sh:   http://www.w3.org/ns/shacl#
xsd:  http://www.w3.org/2001/XMLSchema#
```

`1agent.dev` 是否最终拥有由 owner 确认；在确认前，词表文件放仓库，
实例使用 `urn:1agent:`，不依赖可解析域名。

### 4.2 实例 IRI

| 实体 | IRI 规则 | 说明 |
|---|---|---|
| Session | `urn:1agent:session:<session-id>` | 现有 session id |
| Run | `urn:1agent:run:<run-uuid>` | journal run_id |
| Task | `urn:1agent:task:<run-uuid>:main` | 一个 run 的主任务；子任务可扩展 |
| ToolInvocation | `urn:1agent:tool-call:<run-uuid>:<call-id>` | call_id 只在 run 内唯一 |
| Observation | `urn:1agent:observation:<run-uuid>:<call-id>` | 与 tool call 配对 |
| Claim | `urn:1agent:claim:<run-uuid>:<claim-id>` | claim id 只在 answer 内唯一 |
| Evidence | `urn:1agent:evidence:<run-uuid>:<seq>:<index>` | 现有 evidence 无 id，导出时派生 |
| Answer | `urn:1agent:answer:<run-uuid>` | 一个 run 一个 answer |
| Artifact | `urn:1agent:artifact:sha256:<hex>` | 内容身份优先；path 是属性 |
| Agent | `urn:1agent:agent:<agent-id>` | 人类/主 Agent/队员统一 |
| SquadMember | `urn:1agent:agent:<session-id>:<subagent-id>` | 队员是 Agent 子类 |
| PolicyDecision | `urn:1agent:policy:<run-uuid>:<seq>` | 由 journal/action log 派生 |
| Approval | `urn:1agent:approval:<card-id>` | action card/approval |
| Checkpoint | `urn:1agent:checkpoint:<run-uuid>:<seq>` | journal 中 checkpoint 事件 |

原则：

- **确定性**：同一输入导出同样的 IRI，不使用新随机 UUID。
- **不复用 PID**：进程 ID 只作为 `processId` 观测值，不作为 Agent 身份。
- **内容寻址**：artifact 以 SHA-256 为主身份；路径变化不产生新 artifact。
- **本地 ID 命名空间化**：claim/call 的局部 ID 加上 run 前缀，避免跨 run 冲突。

## 5. 核心词表

### 5.1 类

| 类 | 父类/对齐 | 现有来源 | 说明 |
|---|---|---|---|
| `Agent` | `prov:Agent` | 抽象 | 能执行活动的实体 |
| `HumanOperator` | `Agent` | authority/approval | 人类操作者 |
| `SquadMember` | `Agent` | `SubagentRecord`、`spawn_agent` | 分队队员 |
| `Session` | — | `SessionRecord` | 持久对话容器 |
| `Run` | `prov:Activity` | journal `RunStarted` | 一次有 journal 的执行 |
| `Task` | — | `TaskRequest` | 目标、交付物、约束、预算 |
| `Subtask` | `Task` | `spawn_agent` task、task graph | 队员任务/子任务 |
| `Plan` | — | `RoutePlan`/Master plan | 计划 |
| `Step` | — | journal seq/checkpoint | 可选粒度 |
| `ToolInvocation` | `prov:Activity` | `ToolCalled` | 工具调用 |
| `Observation` | `prov:Entity` | `ToolObserved` | 工具结果 |
| `Evidence` | `prov:Entity` | `Answer.evidence` | 证据 |
| `Claim` | `prov:Entity` | `Answer.claims` | 可验证/推断/假设结论 |
| `Assumption` | — | `Answer.assumptions` | 假设 |
| `Unknown` | — | `Answer.unknowns` | 未知 |
| `Conflict` | — | `Answer.conflicts` | 冲突 |
| `FlipCondition` | — | `Answer.flip_conditions` | 翻转条件 |
| `Answer` | `prov:Entity` | `AnswerProduced` | 结构化答案 |
| `Artifact` | `prov:Entity` | 文件写入/产物/SHA | 产物 |
| `Workspace` | — | Session/Subagent workspace | 工作区 |
| `PermissionSet` | — | `TaskRequest.permissions` | 读/写/网络权限 |
| `AuthorityProfile` | — | `AuthorityProfile` | 授权配置 |
| `PolicyDecision` | — | `ExecutionGate`/action log | allow/approval/deny |
| `Approval` | — | action card/approval envelope | 人工批准 |
| `ActionCard` | — | `browser::policy::ActionCard` | 不可逆动作卡 |
| `Checkpoint` | — | `Checkpoint`/`CheckpointDelta` | 恢复点 |
| `Journal` | — | journal 文件 | 事件序列 |
| `Skill` | — | skill store | 技能 |
| `Experience` | — | experience store | 经验 |
| `Schedule` | — | scheduler | 调度定义 |
| `ScheduledRun` | — | `RunRecord` | 调度执行记录 |
| `ProviderModel` | — | `RouteSelected` | provider/model |
| `Usage` | — | `ModelResponse.usage` | token/cost |
| `Incident` | — | alerts/watchdog | 事故/告警 |
| `EvalCase` / `EvalRun` | — | eval suite/artifacts | 评测 |

### 5.2 关系

| 属性 | 域 → 值域 | 说明 | PROV 对齐 |
|---|---|---|---|
| `partOfSession` | Run → Session | 运行属于会话 | — |
| `hasRun` | Session → Run | 反向 | — |
| `executedBy` | Run/ToolInvocation → Agent | 谁执行 | `prov:wasAssociatedWith` |
| `used` | Run/ToolInvocation → Artifact/Evidence | 使用了什么 | `prov:used` |
| `generated` | Run/ToolInvocation → Artifact/Answer/Observation | 产生了什么 | `prov:generated` |
| `wasGeneratedBy` | Artifact/Answer → Run/ToolInvocation | 反向 | `prov:wasGeneratedBy` |
| `wasDerivedFrom` | Evidence/Claim/Artifact → 来源 | 派生 | `prov:wasDerivedFrom` |
| `wasAttributedTo` | Entity → Agent | 归属 | `prov:wasAttributedTo` |
| `observedBy` | Evidence/Observation → ToolInvocation | 观测来源 | — |
| `supportsClaim` | Evidence → Claim | 支持 | — |
| `contradictsClaim` | Evidence/Claim → Claim | 冲突 | — |
| `cites` | Claim → Evidence/Artifact | 引用 | — |
| `hasEvidence` / `hasClaim` | Answer → Evidence/Claim | 组成 | — |
| `hasUnknown` / `hasAssumption` | Answer → Unknown/Assumption | 组成 | — |
| `hasConflict` | Answer → Conflict | 组成 | — |
| `hasFlipCondition` | Answer/Claim → FlipCondition | 翻转 | — |
| `spawnedBy` | SquadMember → ToolInvocation | 由 spawn_agent 启动 | — |
| `parentOf` / `childOf` | Agent/Run/Session | 父子关系 | — |
| `resumedFrom` | Run → Checkpoint/Run | 恢复自 | — |
| `forkedFrom` | Session → Session | fork 关系 | `prov:wasRevisionOf`（可选） |
| `authorizedBy` | ToolInvocation/Run → AuthorityProfile/PolicyDecision | 授权 | — |
| `decidedBy` | PolicyDecision → Agent | 决策者 | — |
| `approvedBy` | Approval → HumanOperator | 批准者 | — |
| `hasPermission` | Task → PermissionSet | 权限 | — |
| `hasBudget` | Task → Budget | 预算 | — |
| `usedModel` | Run → ProviderModel | 路由结果 | — |
| `hasUsage` | Run → Usage | token/cost | — |
| `hasStatus` | Run/Answer/SquadMember → 状态 | 状态字面量 | — |
| `hasImpact` | Unknown/Assumption → impact | 影响级别 | — |
| `supersedes` | Claim/Answer/Artifact → 旧实体 | 取代 | — |
| `processId` | SquadMember → xsd:integer | 进程观测，不作身份 | — |

### 5.3 状态

本体不把状态和结果混为一谈：

- **RunPhase**：`created → running ⇄ paused → completed | failed | cancelled`
- **RunOutcome**：`completed | needs_evidence | needs_authorization | budget_exhausted | cancelled | provider_error | tool_failure | invalid_output | internal_error`
- **SquadStatus**：`starting | running | completed | failed | timed_out | cancelled`
- **ClaimStatus**：`verified | user_asserted | derived | assumed | model_prior | conflicting | unknown`
- **ApprovalStatus**：`requested | approved | denied | expired | consumed`
- **PolicyOutcome**：`allow | require_approval | deny`

OWL 不负责状态迁移；迁移合法性由 runtime + SHACL/audit 规则校验。
`needs_evidence` 等是 outcome，不是运行 phase；不要在数据里混成一个字段。

### 5.4 运营、动作、安全、身份与时间扩展

| 概念 | 作用 | 现有对应 | 说明 |
|---|---|---|---|
| `ObjectType` / `PropertyType` / `LinkType` | 类型注册表，生成 JSON-LD/SHACL/SQL | journal 事件、session 字段 | 不直接手写 OWL；类型注册表是源 |
| `InterfaceType` / `ValueType` | 可复用接口/值类型 | 现有枚举/结构 | 对应 Palantir interfaces/value types |
| `ActionType` | 动作的可逆性、风险、权限、副作用 | `CuaAction`、`ActionCard` | 本体必须包含“能做什么”，不只“有什么” |
| `ActionInvocation` | 一次动作执行 | `ToolCalled`/`ToolObserved`、action log | 与 ToolInvocation 区分：工具调用可无副作用 |
| `Decision` / `Outcome` | 决策问题与运行结果 | `AnswerProduced`、`RunStopped` | P0 已从 journal 派生；无独立运行时字段 |
| `Option` / `Criterion` | 选项与判断标准 | `ReframeProposed`/`ReframeDecided` | P0 已导出；无 reframe 数据时为空，不伪造 |
| `DataBinding` / `DataSource` | 类型/属性 ↔ 具体数据源 | 无 | 必需；否则本体只是文档 |
| `AccessPolicy` / `Purpose` | 对象/动作级访问与目的约束 | `AuthorityProfile`、`PermissionSet` | 导出必须 permission-aware |
| `Commitment` / `Delegation` | 任务分配、接受、拒绝、完成承诺 | `spawn_agent`、master assign | Super-Agent 的一等实体 |
| `EntityAlias` / `mergedInto` | 实体解析与合并溯源 | 无 | 稳定 ID + 合并 provenance |
| `ConfidenceAssertion` | 置信度及来源 | `ClaimStatus`、provider quality | 没有校准记录就不输出数字，绝不默认 1.0 |
| `validFrom/validTo` | 事实在世界的有效期 | 证据 expires_unix_ms | 与 `observedAt` 区分 |
| `recordedAt` | 我们何时记录 | journal timestamp | 与事件时间区分，支持“当时知道什么” |

**关键修正**：`ActionType`、`DataBinding`、`AccessPolicy` 不是可选装饰；没有它们，
本体无法支撑 Palantir/Fabric 那种“操作层”，也无法解释“谁在什么目的下能做什么”。

## 6. 当前数据 → 本体映射

| 本体概念 | 当前来源 | 现状 | 缺口/建议 |
|---|---|---|---|
| Session | `SessionRecord`（`src/tui_session.rs:19`） | id/title/journals/workspace/archived/preferences | 加 `forkedFromSessionId`（可选） |
| Run | `RunStarted`、journal `run_id` | 有 run UUID | journal 无 session_id；P0 从 session 记录 join |
| Task | `TaskRequest`（`src/domain.rs:19`） | objective/deliverable/constraints/budget | 无 task id；导出派生 `run:main` |
| Permission | `TaskRequest.permissions` | read_roots/network/write | 直接映射 |
| ToolInvocation | `ToolCalled`（`src/journal.rs:141`） | call_id/name/arguments | call_id 需加 run 前缀 |
| Observation | `ToolObserved` | tool/status/summary/content/error | 无 artifact SHA（读）；加 metadata |
| Answer | `AnswerProduced` | status/conclusion/claims/evidence | 无 answer id；派生 run |
| Claim | `Answer.claims`（`src/domain.rs:303`） | id/text/status/sources/scope | id 是 run 内局部；导出加 run 前缀 |
| Evidence | `Answer.evidence`（`src/domain.rs:339`） | source/statement/time/claim_ids | 无 evidence id、无 source kind、无内容 SHA |
| Unknown/Assumption | `Answer` 字段 | 有 impact/why/reduction | 直接映射 |
| Conflict/Flip | `Answer` 字段 | claim_ids/target_ids | 直接映射 |
| SquadMember | `SubagentRecord`（`src/subagent_registry.rs:64`） | task/workspace/journal/status/pid/depth | 缺 parent_session_id、parent_run_id、spawn_call_id、child_run_id |
| Artifact | write/edit metadata `relative_path`/`sha256`/bytes | 写操作有 SHA | read/search 文件缺 SHA |
| AuthorityProfile | `src/authority.rs:38` | actor/scopes/tools/risk/expiry | 不在 journal；需决策关联 |
| PolicyDecision | `ExecutionGate`、action log | 有 allow/require/deny | 未统一写入 journal/observation |
| Approval/ActionCard | `src/browser/policy.rs:79`、approval envelope | 有 card/envelope | 缺 run_id/tool_call_id 关联 |
| Checkpoint | `Checkpoint`/`CheckpointDelta` | seq/state | 直接映射 |
| ScheduleRun | `RunRecord`（`src/schedule_ledger.rs:33`） | schedule_id/run_id/status | 可映射 |
| ProviderModel/Usage | `RouteSelected`、`ModelResponse.usage` | provider/model/cost | 可映射 |
| Skill/Experience | skill/experience store | 版本/状态 | 可映射，v1 可选 |
| ActionLog | `ActionLogEntry`（`src/action_log.rs:93`） | actor/kind/target/outcome/hash | 缺 run/session 关联 |
| ActionType | `CuaAction`/`browser::policy::ActionCard` | 风险/可逆性/描述符 | 导出为类型注册表；补 access policy/purpose |
| DataBinding | `ontology/bindings.example.json` | 映射草案 | P0 落地为版本化 binding 文件 |
| AccessPolicy/Purpose | `AuthorityProfile`/`PermissionSet` | 有权限/范围 | 目的约束不完整；先映射已有字段 |
| Commitment/Delegation | `spawn_agent`/Master assign | 有父子/分配 | 补接受/拒绝/完成承诺；P3 |
| ConfidenceAssertion | 无系统校准记录 | ClaimStatus 不是置信度 | 没有校准就不输出数字 |

### 6.1 Evidence source 的分类规则

现有 `Evidence.source` 是一个字符串，可能同时表示路径、URL、tool id、模型先验。
导出时按以下优先级分类：

1. `http://` / `https://` → `WebSource`
2. 以 `/` 或已知相对路径存在 → `FileSource`（附 `sha256` 若可得）
3. 匹配 `tool:<id>` / observation call id → `ToolObservationSource`
4. 以 `model:` 开头 → `ModelPriorSource`
5. 其他 → `UnknownSource`，保留原文，不猜。

这不是完美的语义解析，但比把字符串直接当事实安全。

## 7. 需要改哪些代码

### P0：类型注册表 + 只读导出（不改运行行为，约 2 周 / 18–20 人日）

1. `ontology/`：Type Registry（object/property/link/action/function/interface/value）、
   Data Binding 草案、JSON-LD context、核心词表、SHACL shapes、示例。
2. 新模块 `src/ontology.rs`（或 `src/ontology/`）：
   - 命名空间/IRI 构造函数；
   - 类型注册表加载与校验（类型名、属性、基数、目标 source）；
   - `export_session(session_root, session_id) -> JsonLdDocument`；
   - `export_run(journal_path) -> JsonLdDocument`；
   - 确定性序列化（同一输入同一字节）；
   - 对 v1/v2 session、旧 journal 的兼容读取。
3. CLI：
   - `1agent ontology types [--json]`
   - `1agent ontology bindings [--json]`
   - `1agent ontology export --session <id> [--format jsonld|json] [--out <path>]`
   - `1agent ontology export --journal <path>`
   - `1agent ontology validate --session <id>`（P0 内部校验，P1 接 SHACL）
4. 测试：
   - golden JSON-LD；
   - 相同输入重复导出字节一致；
   - 缺 journal/坏 JSONL/旧版本 session 不崩溃；
   - 导出不含密钥、不导出完整 prompt；
   - `verify.sh` 不依赖网络。
5. 文档与 feature matrix 标注“只读导出，无运行时依赖”。

P0 **不改**：AgentAnswer schema、模型 prompt、journal 事件、SubagentRecord 字段。
但 P0 必须把 **ActionType/DataBinding/AccessPolicy 的类型定义**放进注册表；
只导出静态对象类型是不够的。

### P1：补 lineage、决策/动作与安全（约 2 周 / 14 人日，向后兼容）

| 文件 | 改动 | 兼容策略 | 验证 |
|---|---|---|---|
| `src/subagent_registry.rs` | `SubagentRecord` 增加 `parent_session_id`、`parent_run_id`、`spawn_call_id`、`child_run_id`，`Option`/default | 旧记录缺失为 `None`；新记录写入 | 旧/新 registry 读取、分队 UI |
| `src/tools/subagent.rs` | 启动前写 parent 信息；child run_id 从 child JSON 回填 | 不改变工具参数 | spawn 测试 |
| `src/tui_session.rs` | `SessionRecord` 增加 `forked_from_session_id` | `#[serde(default)]` | v1/v2 读取、fork 测试 |
| `src/tools/mod.rs` / `files.rs` | `read_file`、`search_files` 的结果 metadata 增加 `sha256`（文件内容可读时） | 新增 metadata 键，不影响模型 | 文件 tool 测试 |
| `src/runtime.rs` | ToolObservation metadata 增加 `policy_outcome`、`policy_reason`、`authority_profile_id`、`approval_card_id`（可用时） | metadata 可空 | gate/approval 测试 |
| `src/action_log.rs` | `ActionLogEntry` 增加可选 `run_id`、`session_id`、`action_type` | default 兼容 | action log 测试 |
| `ontology/bindings.json` | 每个对象/属性 → journal/session/registry 字段的映射与 mappingVersion | 只读；无运行时依赖 | 绑定覆盖率测试 |
| `src/browser/policy.rs` / `src/cua.rs` | 从 `ActionCard`/`CuaAction` 生成 `ActionType`（风险、可逆性、权限） | 只读导出 | action card 测试 |
| `src/authority.rs` | 导出 `AuthorityProfile` 与 `PolicyDecision` 的关联；可选 `AccessPolicy/Purpose` | 只读 | gate/approval 测试 |
| `src/journal.rs` | **暂不新增事件**；session 关系继续由 session.json 的 journals 列表 join | 零协议风险 | 旧 journal 回放 |

P1 仍不要求模型在 answer 里增加字段。导出器从现有结构派生；无法派生的标为 `Unknown`，不伪造。
`ConfidenceAssertion` 只在存在校准记录时输出；没有校准就省略数字，绝不默认 1.0。
Bitemporal 字段（`observedAt`/`recordedAt`/`validFrom`/`validTo`）先由导出器计算，不改运行时契约。

#### P1.1 决策/动作层（新增，1.2 候选）

状态：**P0 + Action Registry + 首个 typed action 已落地（2026-11-19）**：
导出 `Decision`、`Outcome`、`Option`、`Criterion` 节点；`ontology/actions.json`
定义代码变更/发布的 4 个 ActionType；`code.propose_change` 已注册为
typed tool，写 0600 提案 artifact 并经 ExecutionGate，导出
ActionInvocation/Decision.hasAction/Proposal。`run_checks` 写检查报告，
`prepare_release` 在批准+检查通过后写本地 release manifest；人类可通过
`action approve-change` 记录 approve/reject 回执（非模型工具）。`Scenario`
merge 仍待实现。

P1.1 把 1AO 从“审计投影”推进到“可执行决策层”，但不改变 journal 的事实源地位：

1. **类型**：`Decision`、`Option`、`Criterion`、`Outcome`、`ActionType`、`ActionInvocation`、`PolicyDecision`、`Approval`、`Scenario`。
2. **ActionType 注册表**：参数 JSON Schema、前置条件/submission criteria、风险、可逆性、审批策略、副作用、后置条件、幂等键、版本。
3. **Typed tools**：每个写 ActionType 生成一个 ToolSpec/CapabilityManifest；读操作收敛为查询/搜索工具；模型不再自由组合底层写操作。
4. **Decision lineage**：`Decision -> Option -> ActionInvocation -> Policy/Approval -> Outcome`，并绑定 journal run、数据版本、agent/app、时间。
5. **Scenario**：staged changes → diff/影响预览 → 人工批准 → 原子合并；拒绝不产生生产副作用。
6. **Policy**：先做 object-type + action scope，再做 property/cell 级；工具调用前检查对象访问权。

垂直切片建议：文档评审发布或代码变更发布，先做 3–5 个 ActionType。验收：未声明的 Action 无法执行；拒绝/失败无副作用；每个 Action 可回溯到 Decision、Approval、Outcome；1ASH 任务集包含对应决策/动作任务。

### P2：查询、投影与 UI（2–4 周）

- SQLite 投影：`entities(id, type, props_json)`、`relations(src, predicate, dst)`、
  `bindings(type, property, source_path, mapping_version)`；或按需内存图。
  先不做三元组库。
- `1agent ontology query --session <id> --question <cq>`：预置 CQ 查询，
  先给审计/UI 用，不做通用 SPARQL 端点。
- “model once, represent everywhere”：从 Type Registry 生成 JSON-LD、SQLite DDL、
  GraphQL schema（或 Mermaid 图），不手写多套模型。
- TUI：
  - 会话选择器预览增加“证据链完整度/最近 claim”；
  - `Agent-分队` 详情显示 parent session/run/spawn call；
  - 可在详情中打开子 journal（已有）。
- SHACL 校验接入 `ontology validate`；失败只报告，不阻塞运行。
- PROV-O 对齐验证；JSON-LD 1.1 解析器测试。

### P3：Super-Agent 与互操作（4–8 周+）

- 持久队员、跨会话任务、`Commitment`/`Delegation`/接受/拒绝、共享 artifact、
  接管/暂停传播。
- Agent identity/capability/trust；跨机器交换使用 JSON-LD/PROV。
- A2A AgentCard/Task/Artifact/contextId 映射；OpenTelemetry `invoke_agent`/
  `execute_tool` span 导出；与 PROV 互链。
- 只有在 P2 证明查询价值后，才评估 OWL RL/图数据库。

## 8. 方案优劣对比

### 8.1 本体层 vs 现状

| 维度 | 现状 JSON/Schema | 1AO 轻量本体 | 结论 |
|---|---|---|---|
| 消息形状 | 强 | 继续用 JSON Schema | 不替代 |
| 语义关系 | 隐式、靠代码 | 显式、可查询 | 本体赢 |
| 审计溯源 | 需要手写遍历 | 标准关系 + PROV | 本体赢 |
| 跨 Agent/跨版本 | 目录/文件名约定 | 稳定 IRI/词表 | 本体赢 |
| 运行性能 | 无额外成本 | 只读导出，按需 | 持平 |
| 维护成本 | 低 | 中（词表/映射/版本） | 现状赢 |
| 误读风险 | 低（字段明确） | 中（开放世界/推理） | 需约束 |

### 8.2 四种落地方案

| 方案 | 语义表达 | 校验 | 推理 | 查询 | 落地成本 | 运行风险 | 互操作 | 适合 |
|---|---|---|---|---|---|---|---|---|
| A. 现状 JSON + 文档 | 中 | 强 | 无 | 手写 | 低 | 低 | 中 | 当前 0.98 |
| **B. Type Registry + JSON-LD/SHACL + 投影（推荐）** | 高 | 强 | 轻 | 中高 | 中低 | 低 | 高 | 现在到 Super-Agent |
| C. OWL 2 DL + reasoner + 三元组库 | 很高 | 强（开放世界） | 强 | 高 | 高 | 中高 | 很高 | 外部审计/研究/标准交换 |
| D. 属性图数据库 | 中高 | 中 | 中 | 很高 | 中高 | 中 | 中 | 路径查询/大图 |
| E. LLM 自动抽取 KG | 低–中 | 弱 | 弱 | 中 | 中 | 高（幻觉） | 低 | 探索，不可作审计真相 |

推荐 **B 为主，C 为可选导出格式**；不要用 E 生成事实层。

### 8.2b 国际实践对“优劣”的修正

| 实践 | 值得借鉴 | 不值得照搬 |
|---|---|---|
| Palantir | 对象/关系/动作/函数/安全一体的 operational ontology；action type 与 dynamic security | 全套平台复杂度；先做我们的类型注册表，不复制平台 |
| Microsoft Fabric | entity key、data binding、静态/时序属性、关系类型；生成 ontology | 依赖 OneLake/Eventhouse 的绑定模型；我们是 journal/session 绑定 |
| Netflix UDA | RDF+SHACL upper metamodel；named graph；投影到 GraphQL/Avro/Iceberg/SQL | 一开始就做多投影/全量 RDF 迁移；我们先用 SQLite/JSON-LD |
| LinkedIn | 稳定 URN、typed edges、confidence、实体解析 | 自研大图 + ML 实体解析的团队成本；先做确定性别名 |
| Uber / Atlas | 技术元数据与业务本体分层；lineage；classification | 把运行元数据硬塞进业务本体；先分开 |
| Google KV | 概率融合、provenance、LCWA | 没有校准就用概率/置信度；我们不默认数字 |
| FIBO | 模块化 OWL、行业治理、operational ontology | 全行业公理/重型 DL；先做 1Agent profile |
| GraphRAG/LLM | 非结构化证据抽取、社区摘要、候选映射 | 把 LLM 抽取结果直接当审计事实；必须人工/测试复核 |

结论：**“类型注册表 + 数据绑定 + 动作/安全 + 轻量语义投影”** 比“纯 OWL 词表”
更接近大厂的实际落地方式，也比“只读 RDF 导出”更能支撑 Super-Agent。

### 8.3 导出式 vs 双写式

| 方式 | 一致性 | 运行风险 | 复杂度 | 结论 |
|---|---|---|---|---|
| 只读导出/投影 | 最终一致；可重算 | 低 | 低 | **推荐 P0/P1** |
| 运行时双写 | 强一致但事务复杂 | 中高 | 高 | 不做 |
| 事件溯源投影 | 可由 journal 重建 | 低 | 中 | P2 可做 |

### 8.4 OWL 与 SHACL 的分工

- OWL：类层次、域/值域、等价/不相交、可选推理，适合外部理解和一致性检查。
- SHACL：必填、枚举、基数、时间顺序、IRI 存在性，适合操作校验。
- 用 OWL 的开放世界做“缺字段 = 非法”会误判；用 SHACL 做“概念推理”会过度约束。
- 两者都保留，但运行时不依赖推理机。

## 9. 示例（JSON-LD）

```json
{
  "@context": "https://1agent.dev/ontology/0.98/context.jsonld",
  "@id": "urn:1agent:run:5f0c...",
  "@type": ["one:Run", "prov:Activity"],
  "partOfSession": {"@id": "urn:1agent:session:ses_..."},
  "executedBy": {"@id": "urn:1agent:agent:deepseek-primary"},
  "startedAt": "2026-11-15T12:35:00+08:00",
  "hasTask": {
    "@id": "urn:1agent:task:5f0c...:main",
    "@type": "one:Task",
    "objective": "修复 setup 模型选择 bug",
    "hasPermission": {
      "@type": "one:PermissionSet",
      "allowWrite": true,
      "allowNetwork": false
    }
  },
  "generated": {"@id": "urn:1agent:answer:5f0c..."},
  "used": [
    {"@id": "urn:1agent:tool-call:5f0c...:call-1"},
    {"@id": "urn:1agent:artifact:sha256:abc..."}
  ]
}
```

Claim/Evidence 片段：

```json
{
  "@id": "urn:1agent:claim:5f0c...:c1",
  "@type": "one:Claim",
  "claimText": "configured model wins over the first advertised model",
  "claimStatus": "verified",
  "cites": [
    {"@id": "urn:1agent:evidence:5f0c...:12:0"}
  ]
}
```

```json
{
  "@id": "urn:1agent:evidence:5f0c...:12:0",
  "@type": "one:Evidence",
  "sourceKind": "file",
  "sourceRef": "src/onboarding.rs",
  "sha256": "…",
  "statement": "resolve_setup_model prefers configured model",
  "observedAt": "2026-11-15T12:36:00+08:00",
  "supportsClaim": {"@id": "urn:1agent:claim:5f0c...:c1"}
}
```

动作与绑定片段：

```json
{
  "@id": "urn:1agent:action:5f0c...:card-1",
  "@type": "ActionInvocation",
  "hasActionType": {"@id": "one:ActionType/browser-click"},
  "hasPolicyDecision": {"@id": "urn:1agent:policy:5f0c...:44"},
  "hasPurpose": {"@id": "urn:1agent:purpose:operator-approved"},
  "status": "approved"
}
```

```json
{
  "@id": "one:DataBinding/run-partOfSession",
  "@type": "DataBinding",
  "boundTo": {"@id": "one:DataSource/session-json"},
  "sourcePath": "session.json#journals",
  "mappingVersion": "1"
}
```

`ActionType` 至少包含 `risk`、`reversible`、`hasAccessPolicy`；没有这些字段，
本体无法解释“这次点击为什么被批准/拒绝”。

## 10. 明确定义“完成”的标准

P0/P1 完成需要同时满足：

1. 16 个 CQ 中至少 12 个可由导出查询回答；其余有明确缺口记录。
2. `1agent ontology export` 对同一输入字节级确定；golden test 通过。
3. 旧版 session/journal 可导出；坏 journal 不 panic、不静默丢证据。
4. 导出文件不含密钥、原始 prompt、完整文件内容；有 redaction 测试。
5. SHACL/内部校验能发现：claim 无 evidence、evidence 指不存在的 claim、run 无 outcome、squad 无 parent、artifact sha 格式错误。
6. 0 运行时行为变化；`verify.sh` 仍全绿；导出失败不影响任务。
7. 每个不可逆 `ActionCard` 导出都有 `ActionType` + policy/approval 关联，无法关联时显式 `Unknown`。
8. Artifact 以 SHA-256 稳定识别；alias/merge 有 provenance；PID 不出现在任何身份 IRI。
9. 导出记录同时有 `observedAt` 与 `recordedAt`；可用时输出 `validFrom/validTo`。
10. JSON-LD 可解析、PROV 术语可校验；A2A/OTel 映射作为可选投影，不阻塞 P0。
11. 文档、Obsidian 镜像、feature matrix 同步。

## 11. 风险与反向意见

| 风险 | 影响 | 缓解 |
|---|---|---|
| 词表与代码漂移 | 导出失真 | 从 Rust 结构生成/校验词表；golden tests |
| 过度建模 | 工期膨胀 | 只做 16 CQ；P0 不建 Step/Incident 细节 |
| OWL 开放世界误判 | 错误拒绝/结论 | SHACL 做操作约束；推理仅离线 |
| Evidence source 字符串歧义 | 错误归因 | sourceKind 显式化；无法判断标 Unknown |
| PID 当身份 | 重启后错链 | PID 仅 processId 观测；Agent 用稳定 ID |
| 子 Agent 缺 parent 字段 | 无法回溯 | P1 加 parent_session/run/spawn_call |
| 导出泄密 | 安全事件 | 0600；只导出引用/哈希；redaction 测试 |
| 把模型答案当事实 | 虚假审计 | 保留 claimStatus/assumption/unknown；本体不做真值推断 |
| 双写不一致 | 数据分裂 | 只读投影，journal 唯一事实源 |
| 版本升级破坏旧数据 | 回放失败 | 导出器兼容 v1/v2；不新增必填字段 |
| ActionType 与真实 tool 行为漂移 | 审批语义错误 | 从 CuaAction/ActionCard 生成 + 测试；版本化 |
| DataBinding 漂移 | 导出字段错位 | bindingVersion + golden tests + 覆盖率检查 |
| 置信度被误读为概率 | 虚假确定性 | 无校准不输出；保留 claimStatus；人工复核 |
| 权限/目的导出泄漏 | 安全事件 | permission-aware export；sensitivity/purpose；redaction |
| Bitemporal 过度复杂 | 查询难用 | 先导出 observedAt/recordedAt；valid time 仅证据/审批 |

**反对意见**：如果 1Agent 近期只需要“能回放、能审计”，现有 JSON Schema + journal 已够；本体的收益主要在跨会话/跨 Agent/外部交换和复杂查询。若团队没有明确的 CQ 消费者，P0 之后应停，不要继续加 OWL/图库。

## 12. 开放问题（需要 owner 决策）

1. `1agent.dev` 是否由我们控制？否则实例继续用 `urn:1agent:`。
2. Session↔Run 连接：P0 用 session.json join，还是 P1 加 `SessionLinked` journal 事件？我建议先 join。
3. Claim/Evidence 是否升级 answer schema v3，加入 `evidence_id`/`source_kind`/`sha256`？我建议**先导出派生，不改模型契约**。
4. Artifact 身份以内容 SHA 为主，路径只作属性；是否接受 rename 不产生新 artifact？
5. 是否需要 OWL RL 外部一致性检查，还是 SHACL 足够？
6. 队员的持久身份：PID + workspace 不够；需要 session-scoped ID 还是全局 agent ID？
7. 是否将 policy decision/approval 写入 journal，还是保持 action log + metadata 关联？
8. `ActionType` 以 `CuaAction`/`ActionCard` 为源，还是单独维护类型注册表？我建议“代码生成 + 人工补充”。
9. `DataBinding` 的 owner 是 ontology 团队还是各模块？没有 owner 的 binding 必然漂移。
10. 是否在本体层表达 `Purpose/AccessPolicy`，还是只引用现有 authority 配置？
11. 是否引入 A2A AgentCard/Task 映射，还是先保持内部本体、外部另做适配层？
12. 置信度只在有校准记录时输出；谁负责校准与版本化？
13. `Decision` 是一等实体，还是 P0 先从 Answer 派生、P1.1 再升级为独立实体？我建议后者。
14. P1.1 的参考垂直切片选“文档评审发布”还是“代码变更发布”？两者都要有 oracle 和审批链。

## 13. 交付物与下一步

本仓库已放：

- `ontology/1agent-context.jsonld`：JSON-LD 1.1 context 草案
- `ontology/1agent-core.ttl`：核心类/属性 + PROV 对齐草案
- `ontology/1agent-shapes.ttl`：SHACL 形状草案
- `ontology/examples/run.jsonld`：一个 run/answer/evidence/claim 示例

建议下一步顺序：

1. owner 评审本文 0.3 与第 12 节；
2. 只读 exporter + golden tests（P0）——已实现 Decision/Outcome/Option/Criterion/Proposal；
3. P1 lineage 字段（parent/session/run/sha 等）；
4. P1.1 代码发布垂直：propose/checks/approve/release 已本地走通；下一步补负例、action_version 与 capability manifest 选择；
5. 用 16 个 CQ + 新增决策/动作 CQ 做真实会话导出验收；
6. **负例全绿后**再做 Scenario patch/worktree/merge；
7. 再决定是否进入 SQLite/图查询或 OWL。

不要先做：全量 OWL 建模、图数据库迁移、LLM 自动抽取知识图谱、没有 Action 的“本体大屏”。

## 14. 参考来源（公开资料，2026-11 阅读）

**大厂/平台实践**

- Palantir Foundry Ontology — Types reference: <https://www.palantir.com/docs/foundry/object-link-types>
- Palantir Foundry — Core concepts: <https://www.palantir.com/docs/foundry/ontology/core-concepts>
- Microsoft Fabric — Ontology glossary: <https://learn.microsoft.com/fabric/iq/ontology/resources-glossary>
- Microsoft Fabric — Bind data: <https://learn.microsoft.com/fabric/iq/ontology/how-to-bind-data>
- Netflix Tech Blog — UDA / Upper: <https://netflixtechblog.com/uda-unified-data-architecture-6a6aee261d8d>
- LinkedIn Economic Graph — data explainer: <https://economicgraph.linkedin.com/content/dam/me/economicgraph/en-us/PDF/linkedin-ddp-data-explainer-June2022.pdf>
- Uber — Databook: <https://www.uber.com/blog/databook/>
- Apache Atlas — Type system / architecture: <https://atlas.apache.org/>
- EDM Council — FIBO: <https://spec.edmcouncil.org/fibo>
- Google — Knowledge Vault (KDD 2014): <https://dl.acm.org/doi/10.1145/2623330.2623623>

**标准与论文**

- W3C — PROV-O: <https://www.w3.org/TR/prov-o/>
- W3C — SHACL: <https://www.w3.org/TR/shacl/>
- W3C — OWL 2 Overview: <https://www.w3.org/TR/owl2-overview/>
- Noy & McGuinness — Ontology Development 101 (2001): <https://protege.stanford.edu/publications/ontology_development/ontology101.pdf>
- Grüninger & Fox — Methodology for the Design and Evaluation of Ontologies (1995, TOVE): <https://www.cs.toronto.edu/~gruninger/publications/1995-gruninger-fox.pdf>
- Guarino & Welty — Evaluating Ontological Decisions with OntoClean (CACM 2002): <https://dl.acm.org/doi/10.1145/503124.503150>
- Chekol & Stuckenschmidt — Bitemporal Knowledge Graphs (TIME 2019): <https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.TIME.2019.8>
- Boella, Damiano, Hulstijn, van der Torre — Common ontology of agent communication / social commitments: <https://www.sciencedirect.com/science/article/pii/S1570826809000290>
- Edge et al. — From Local to Global: A Graph RAG Approach (Microsoft Research, 2024): <https://arxiv.org/abs/2404.16130>
- Large Language Models for Ontology Engineering: A Systematic Literature Review (2025): <https://www.semantic-web-journal.org/content/large-language-models-ontology-engineering-systematic-literature-review-1>
- A2A — Agent-to-Agent Protocol: <https://agent2agent.info/>
- OpenTelemetry — GenAI semantic conventions: <https://opentelemetry.io/docs/specs/semconv/gen-ai/>

> 说明：部分大厂细节来自官方文档摘要/公开演讲/二手技术分析；正式立项前应让 owner
> 或架构评审直接核对原始文档。论文结论用于方法校准，不是产品合规证明。
