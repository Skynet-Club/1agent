# 评估体系 · Evaluation

1Agent 的评估不是单一分数，而是三层 + 过程指标 + 失败归因 + 重复采样。

## 1. 三层度量

| 层 | 1Agent 指标 | 实现 |
|---|---|---|
| 结果质量 | passed/failed、rubric、evidence/claims/unknowns、decision completeness、hard gate | `src/eval.rs` `EvalCaseResult`、`EvalExpectation` |
| 执行过程 | provider calls、tool calls、failed tool calls、redundant tool calls、time to first tool、duration、usage、stop reason | 从 run journal 派生，写入 `cases.jsonl` |
| 稳定可靠 | 重复运行通过率、pass^k、至少一次通过、crash-free、completed replay、benchmark gate | `eval --repeats`、`benchmark-run`、`src/benchmark.rs` |

`pass^k` 在这里定义为**同一 case 的 k 次尝试全部通过**；`pass_at_least_once` 表示至少一次通过。两者分开报告，避免用“平均成功率”掩盖偶发失败。

## 2. 过程指标

`EvalCaseResult` 新增：

- `tool_calls`：工具调用总数
- `failed_tool_calls`：非 `Ok` 的工具观察数
- `redundant_tool_calls`：同工具、同参数的重复调用
- `time_to_first_tool_ms`：运行开始到第一次工具调用的时间
- `provider_calls`：模型请求次数
- `usage`：token 与成本

这些指标从 journal 派生，不需要模型额外输出。

## 3. 失败分类

`FailureClass` 是规则分类，不是 LLM judge：

```text
planning / tool_selection / tool_arguments / evidence /
generation / budget / safety / provider / infra / unknown
```

分类来自显式检查文本，例如：

- `expected at most 4 tool calls` → `budget`
- `forbidden tool` / `required tool` → `tool_selection`
- `tool error code` / `invalid arguments` → `tool_arguments`
- evidence / claim / unknown / conflict → `evidence`
- audit / replay / journal / workspace → `infra`
- permission / authorization / high-risk → `safety`
- stop reason / answer status → `generation`

报告同时给出 `failure_class_counts`，可按类别聚合优化。

## 4. 重复运行

```bash
# 同一 suite 跑 3 次，报告 pass^3 和每例通过率
1agent eval --suite evals/agent-loop-v1.json --config ~/.1a/config.json \
  --repeats 3 --artifacts-dir docs/evidence/agent-loop-repeat

# 要求所有 case 的 pass^3 达到 100%
1agent eval ... --repeats 3 --min-pass-power-k-bps 10000
```

输出 `repeat-report.json`：

```json
{
  "repeats": 3,
  "total_cases": 100,
  "total_attempts": 300,
  "overall_pass_rate_bps": 9900,
  "pass_power_k_bps": 9700,
  "pass_at_least_once_bps": 10000,
  "failure_class_counts": {"budget": 2, "evidence": 1},
  "cases": [
    {"id": "chat-19", "attempts": 3, "passes": 2, "pass_rate_bps": 6666,
     "pass_power_k": false, "failure_classes": ["budget"]}
  ]
}
```

## 5. 人工校准

规则 rubric 不能判断“内容是否真的有用”。人工黄金集应该是独立标注的：

- 30–50 条，覆盖六类任务；
- 标注可用性、事实正确性、过度追问、是否该拒绝；
- 与规则分、模型分做一致性对比；
- 只有校准过的指标进入 release gate。

**当前状态：未完成。** 0.97 的标签是 AI/批量写入，不是人工盲评；不要把两者混为一谈。

## 6. 尚未关闭的评估门

- 8h DeepSeek 模型质量（需要成本/时间批准）
- R1–R9 真实用户指标（需要真实 cohort）
- 单 case 多种子/多次运行的统计显著性
- 语义质量的人工校准

代码级评估通过不等于产品 1.0 GA。
