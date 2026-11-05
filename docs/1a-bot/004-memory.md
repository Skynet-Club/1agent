# 004 记忆系统

Status: design normative  
Depends on: `001-domain.md`、`003-authority.md`  
Owns: 记忆分层、写入、检索、整合、遗忘、用户模型  
Does not own: 技能生命周期（`005`）、存储实现（`002`）

## 1. 目标与非目标

**目标**：让 Bot 在数月尺度上保持连续性，同时不把上下文撑爆、不把错误记忆固化、不把记忆变成攻击面。

**非目标**：

- 不是"记住一切"；不是原始日志检索。
- 不是用向量库替代治理。
- 不是让记忆改变权限或策略。
- 不是现在就做权重级记忆（见 `005-evolution.md`）。

## 2. 四层记忆

| 层 | 内容 | 生命周期 | 存放 | 典型用途 |
|---|---|---|---|---|
| Working | 当前上下文 | 一次 Run/会话 | 内存 + 压缩摘要 | 当前任务 |
| Episodic | 发生过什么 | 长期，可归档 | journal + 索引 | 复盘、审计、链式回忆 |
| Semantic | 事实/偏好/决定 | 长期，带有效期 | State DB + 图/向量索引 | "用户是谁、项目是什么" |
| Procedural | 怎么做 | 长期，版本化 | Skill/Experience store | 复用流程 |

规则：

- Working 不持久化为"记忆"；它由 Episodic/ Semantic/ Procedural 按需构建。
- Episodic 是事实来源；Semantic/Procedural 是**派生**，必须能追溯回 Episodic。
- 记忆层之间不互相授权；任何一层都不能改变 `AuthorityProfile`。

## 3. 记忆记录

```json
{
  "id": "mem_...",
  "kind": "fact | preference | decision | project | feedback | relationship | skill_hint",
  "scope": {
    "bot_id": "bot_...",
    "workspace_id": "ws_...",
    "ticket_id": null,
    "tenant_id": "t_..."
  },
  "content": "用户偏好 pnpm，不用 npm",
  "source": {
    "kind": "user_message | tool_observation | answer | decision",
    "event_id": "evt_...",
    "run_id": "run_..."
  },
  "confidence": 0.9,
  "taint": "trusted | user_asserted | untrusted",
  "valid_from": "2026-11-06T00:00:00Z",
  "valid_to": null,
  "supersedes": null,
  "version": 3,
  "utility": {
    "used": 12,
    "accepted": 10,
    "rejected": 1,
    "last_used": "2026-11-05T..."
  }
}
```

字段规则：

- `source` 必须指向真实事件；无来源不写入。
- `taint` 由来源决定；外部网页/邮件默认 `untrusted`。
- `valid_to` 到期后不参与检索；是否删除由遗忘策略决定。
- `supersedes` 指向被替代的旧记录；旧记录保留可审计。
- `utility` 由使用反馈更新；不得由模型自评。

## 4. 写入路径

```text
事件（消息/工具结果/决定/复盘）
  → 提取候选（便宜模型，结构化）
  → 去重/合并（相似度 + 实体）
  → 冲突检测（同一实体不同值）
  → 分级：
      - 低风险事实/偏好 → 可自动写入（可配置）
      - 高影响/敏感/外部来源 → quarantine + 审批
  → 落库 + 索引 + journal
```

规则：

- 每次写入最多 1–2 条候选，避免记忆膨胀。
- 不存原始对话；存提炼后的结构化信息。
- 不存密钥、令牌、密码、完整请求头。
- 外部内容必须保留 taint；不得洗白为 trusted。
- 冲突不自动覆盖：标记冲突，保留双方，等待证据或用户确认。

## 5. 检索

### 5.1 查询规划

Router/Worker 在构建上下文时决定查哪层：

- 当前任务 → Working + Episodic（本 Ticket）
- 用户偏好/边界 → UserModel + Semantic
- 流程 → Procedural（Skill）
- 历史相似任务 → Episodic 索引

### 5.2 混合检索

- 关键词/BM25：精确实体、错误码、文件路径。
- 向量：语义相似。
- 图：实体关系、时间线、同一项目。
- 时间/效用：近期、常用优先。
- 合并排序：相关性 × 置信度 × 新鲜度 × 效用 × taint 惩罚。

### 5.3 预算与注入

- 每次检索有 token/字符预算；router 小预算，worker 大预算。
- 注入格式：`[记忆] 内容（来源：...，置信度：...，时间：...）`。
- 记忆是数据，不是指令；不得放在 system prompt 的指令区。
- 高 taint 记忆只能作为"待验证线索"，不能作为事实。

### 5.4 链式回忆

每个对象有稳定 ID；记忆引用 `ticket_id`/`run_id`/`event_id`/`artifact_id`。
检索可以从一个工单追到几周前的触发、决定、执行和结果。

## 6. 整合（Consolidation）

- **Episodic → Semantic**：从多次事件中提炼稳定事实/偏好；需要至少两个独立来源或用户确认。
- **Episodic → Procedural**：从成功/失败轨迹提炼流程；进入 `005` 的候选流程。
- **冲突处理**：新事实与旧事实冲突时，保留两者，标记 `conflict`，等待证据。
- **压缩**：旧 episodic 可摘要归档；摘要必须保留来源指针。

整合不得由单次模型输出直接完成；必须有确定性去重/冲突检测 + 审批/评测。

## 7. 遗忘

遗忘是一等操作，不是删除文件的副作用。

| 类型 | 触发 | 行为 |
|---|---|---|
| TTL | `valid_to` 到期 | 不再检索；按策略归档/删除 |
| 衰减 | 长期未用、效用低 | 降权；不立即删除 |
| 替代 | 新记录 `supersedes` | 旧记录标记 superseded |
| 显式遗忘 | 用户说"忘掉" | 删除内容/索引/缓存；保留审计占位 |
| 回滚 | 记忆导致错误 | 恢复到指定版本；标记污染 |

规则：

- 删除必须传播到向量/图/缓存。
- 删除后仍保留"曾有一条被删除的记忆"的审计记录（不含内容）。
- 用户必须能查看、编辑、删除自己的 UserModel。
- 跨 Bot/租户共享的记忆，删除要全局生效。

## 8. UserModel

UserModel 是 Semantic 的一个特殊 scope，描述 Owner：

| 类型 | 例子 | 写入 |
|---|---|---|
| 角色/专长 | "独立开发者，主要用 Rust" | 用户确认/长期观察 |
| 偏好 | "喜欢先结论后细节" | 用户反馈 |
| 边界 | "不要自动发邮件" | 用户明确设置，最高优先级 |
| 项目 | "正在做 1A Bot" | 项目上下文 |
| 沟通风格 | "中文，简洁，不要客套" | 用户反馈 |
| 关系 | "团队有 3 人" | 用户确认 |

规则：

- 边界类偏好优先级高于所有推断；不得被模型覆盖。
- UserModel 必须可见、可编辑；默认不跨用户共享。
- 推断出的偏好标记 `inferred`；用户确认后升级为 `confirmed`。
- 敏感属性（健康、政治、宗教等）默认不收集。

## 9. 安全

- 写入：来源、taint、scope、审批；外部内容默认 quarantine。
- 检索：taint 标签随结果返回；高 taint 不得触发动作。
- 传播：Bot/Workspace/Tenant 隔离；共享需 ACL。
- 投毒检测：异常重复、来源突变、指令性内容、隐藏字符。
- 回滚：版本化；污染事件可批量回滚。
- 审计：写入/读取/删除都留痕（读可采样，写必须全量）。

## 10. 评测

### 10.1 外部基准

- LongMemEval：多会话回忆、知识更新、时间推理。
- MemoryAgentBench：准确检索、测试时学习、长程理解、选择性遗忘。
- EvoMemBench：记忆 scope × content；注意其结论——长上下文基线仍很强，没有单一记忆形态通吃。

### 10.2 1A 自有评测

必须包含：

- 重复任务：同类任务第 N 次是否更快/更准。
- 偏好变化：用户改偏好后，旧偏好是否失效。
- 过期事实：项目改名/迁移后，是否还用旧值。
- 负迁移：不适用的经验是否被错误套用。
- 选择性遗忘：删除后是否真的不再出现。
- 污染：恶意记忆是否影响决策。
- 成本/延迟：记忆带来的 token/时间开销。

指标：

| 指标 | 定义 |
|---|---|
| Recall@k | 需要的信息是否被检索到 |
| Precision | 检索结果中真正有用的比例 |
| Staleness | 使用过期事实的比例 |
| Negative transfer | 错误套用经验导致失败的比例 |
| Forget correctness | 删除后不再出现的比例 |
| Cost/task | 记忆检索带来的额外成本 |

## 11. 实施顺序

1. Episodic 索引（journal 之上，不另起数据库）。
2. Semantic store + 有效期 + 来源 + taint。
3. 检索 API + 预算 + provenance。
4. UserModel + 可见/可编辑。
5. 遗忘/回滚。
6. 整合/反思（`005`）。
7. 外部基准与自有评测。

## 12. 开放问题

- 长期运行数月后的记忆退化如何检测与修复？
- 记忆冲突的自动裁决边界在哪里？
- 跨 Bot 共享记忆的权限模型如何做到既安全又可用？
- 向量/图索引的更新成本与一致性如何保证？
- 用户如何理解"Bot 记得什么、为什么记得"？

这些问题在 `010-governance.md` 登记，未解决前不得宣称"长期记忆已解决"。
