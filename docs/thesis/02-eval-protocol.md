# 需求重构评测协议 v0（预注册）

Date: 2026-11-04
Status: pre-registered; cases not yet collected
Freeze: 见 §9（写入后计算 sha256，此后修改必须新版本）

## 1. 目的与不可回避的前提

本协议检验的唯一命题：

> 在用户请求实际是"手段"的任务上，regress-then-solve 相比"直接执行字面请求"，
> 能提高用户接受率并降低总成本。

前提：**"用户真正想要什么"不可观测。** 因此本协议不标注"真实需求"，只标注
**行为结果**（接受 / 拒绝 / 拒绝类别）。任何要求标注"真实需求"的设计都是伪造
ground truth，一律不采用。

## 2. 语料来源（按优先级）

1. **真实请求**：从 `.journal/*.jsonl` 抽取，当前仅约 10 条真实请求，不足；
2. **用户访谈记录**：laddering + Critical Decision Method 产出的一手转写；
3. **Critical Incident Technique**：只收具体失败事件，不收观点；
4. **合成案例**：仅用于扩展覆盖面，且必须由两人独立确认"确属 means 请求"。

当前规模不足，因此本套件初始状态为 `execution_status: template`，不允许计入
任何产品质量声明。

## 3. 案例结构

```json
{
  "id": "drill-001",
  "stated_request": "我要买一个钻头",
  "classification_expected": "means",
  "context": {
    "constraints": ["墙面为承重墙", "不可改动结构"],
    "permissions": {"allow_write": false},
    "known_facts": ["两个房间各有一台电脑", "需要同时上网"]
  },
  "reference_hops": [
    {"from": "买钻头", "to": "在墙上打孔"},
    {"from": "打孔", "to": "把网线穿过去"},
    {"from": "穿线", "to": "两个房间的电脑都能联网"}
  ],
  "acceptable_substitutes": ["无线路由器", "电力猫", "Mesh"],
  "unacceptable_substitutes": ["拆墙", "拉明线经过客厅"],
  "domain_traps": ["承重墙不可钻孔", "房东不许可"],
  "reframing_block": {
    "rubric": "acceptance_only",
    "labels": ["accept", "reject_cost", "reject_constraint",
               "reject_literal", "reject_unverifiable"]
  }
}
```

`reference_hops` 仅用于研究与评分辅助，**不作为 ground truth**；评分只看标签。

## 4. 标注协议

- 标注者：≥2 人，且**不得是系统设计者**。
- 每人独立标注，禁止讨论后再改。
- 每条案例标三件事：
  1. 该请求是否属于 means（是/否/不确定）；
  2. 对系统给出的替代方案：accept / reject；
  3. 若 reject，归类：成本转移 / 约束未满足 / 我就是要原方案 / 不可验证 / 其他。
- 一致性门槛：**Cohen's κ ≥ 0.6**（substantial）。低于 0.6 则案例定义有问题，
  先修案例，不修系统。
- 复用现有 `src/annotation.rs`（已实现 `kappa_millis`、observed/expected bps），
  不新写统计代码。

## 5. 对照组（唯一能排除"多花 token 而已"的设计）

同模型、同案例、同预算：

- **A 组（baseline）**：直接执行 `stated_request`。
- **B 组（treatment）**：regress-then-solve，含决策门。

盲评：标注者不知道哪条输出来自哪组；顺序随机化。

## 6. 指标与门槛

| 指标 | 定义 | 门槛（初始） |
|---|---|---|
| `obviation_rate` | 原请求被替代方案解决的比例 | ≥ 0.25 |
| `acceptance_rate` | 替代方案被接受的比例 | B 组 > A 组，且差值 ≥ 0.15 |
| `regret_rate` | 用户坚持原方案的比例 | ≤ 0.20 |
| `precision_of_means` | 被判为 means 且标注者同意的比例 | ≥ 0.80 |
| `cost_delta` | 相对 A 组的钱/时间差 | < 0（B 更省） |
| `unverifiable_rate` | 提出的更高层需求用户无法判断真假的比例 | ≤ 0.10 |
| κ | 标注一致性 | ≥ 0.60 |

门槛为初始设定，**只能凭记录在案的证据修订**，不得事后调参以通过。

## 7. 因果检验

对每个被接受的替代方案，检查 `need ⊥ original_means | substitute`。
不满足 d-separation 的案例计入 `substitute_hallucination_rate`，门槛 ≤ 0.05。

研究阶段用 dagitty 人工核验；稳定后固化为回归夹具。

## 8. 红队证伪

独立 agent，输入仅：`stated_request` + `context` + 系统给出的重构。
任务：用 `domain_traps` 或任何合理的现实约束推翻该重构。

判定：若红队给出系统未预先反驳的反例，该案例判失败。
此检查制度化"找反例"，不依赖评审者自觉。

## 9. 冻结方式

1. 本文件与案例集写入后计算 SHA-256：
   `sha256sum docs/thesis/02-eval-protocol.md`
2. 摘要记入 `docs/thesis/FREEZE.txt`，并附案例集摘要。
3. 任何修改产生 `v1`，旧版本保留，不覆盖。

## 10. 杀项条件（一旦触发即停止，不是继续调参）

1. `acceptance_rate` 差值 < 0.05：命题不成立。
2. `regret_rate` > `obviation_rate`：重构的伤害大于收益。
3. `unverifiable_rate` > 0.20：回溯产出哲学化内容，方法不可用。
4. `precision_of_means` < 0.60：分类器不可用于默认路径。
5. 真实语料中 means 类占比 < 10%：产品价值面过窄。
6. 红队推翻率 > 0.30：重构不稳健。

## 11. 明确不声称的内容

- 不声称模型的"需求理解"正确——只声称行为结果。
- 不声称跨 provider 可迁移——未测。
- 不声称优于人类专家——未设该对照。
- 不声称该指标可外推到其他产品形态。
