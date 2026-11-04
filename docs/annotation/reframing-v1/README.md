# 需求重构语义标注 v1

这份文档给**两位独立标注者**看。你要判断的不是代码，是"这个回答对不对"。

## 1. 你会看到什么

`items.jsonl` 每行一条，共 22 条。每条包含：

```json
{
  "case_id": "h2-means-nas-08::ask",
  "stated_request": "推荐个 NAS",
  "constraints": ["预算五千"],
  "model": { "classification": "means", "question": "...", "options": [...], "substitutes": [...] },
  "question_for_labeler": "助手在这个请求上追问了一个问题。应该追问吗？"
}
```

每个 case 有一条 `::ask`；只有被判定为手段的 case 另有一条 `::substitute`（目的类没有替代方案，问"没有替代是否可接受"是废问题）：

- `<case>::ask` —— 助手在这个请求上**该不该追问**
- `<case>::substitute` —— 助手给出的替代方案**用户会不会接受**

## 2. 你要判断什么

每个 `::ask`：

- 助手追问了 → 这个请求**该不该**被追问？
- 助手没追问 → 这个请求**本不该**被追问吗？

每个 `::substitute`：

- 有替代方案 → 至少有一个是**用户会接受**的吗？
- 没有替代方案 → 这个"不提替代"是**可接受**的吗？

## 3. 三个标签

只能选一个：

| 标签 | 含义 |
|---|---|
| `supported` | 判断为"对"：该追问 / 该沉默 / 替代可接受 |
| `unsupported` | 判断为"错"：不该追问 / 该追问却没问 / 替代不可接受 |
| `needs_evidence` | 信息不足，两种判断都说得通 |

**没有"差不多"。** 拿不准就用 `needs_evidence`——它本身是有用的信号：说明条目写得不够清楚。

## 4. 填写方式

只给标注者一份**自包含**的表格，不要让他去看 jsonl：

```bash
python3 scripts/reframing/make_label_sheets.py \
  --items docs/annotation/reframing-v1/items.jsonl \
  --out ~/下载/1A-标注材料
```

生成的 `01-标注表-A.csv`（与 B 相同）每行包含原请求、助手做了什么、要你判断，
以及一个待填的 `label`。标注者把 `?` 换成三个词之一即可，不需要交叉查文件。

导入时按表头取列，多余的列和顺序变化都不影响：

```bash
python3 scripts/reframing/import_labels.py --sheet labels-A.csv --annotator alice --store ./annotation-store
python3 scripts/reframing/import_labels.py --sheet labels-B.csv --annotator bob   --store ./annotation-store
./target/release/first-principles-agent annotate agreement --store ./annotation-store --json
```

`?` 或空白的行会被跳过并计数，所以可以分批交。

## 5. 两条硬规则

1. **独立性**：两人各自填完之前不讨论。讨论后再改，κ 就失去意义。
2. **盲性**：不要看 `fixtures/reframing/`、`docs/evidence-*.md`、`docs/claims.md`。那里有我们对答案的预期。

## 6. 结果怎么读

输出里的 `kappa_millis` 就是 Cohen's κ（×1000）：

```json
{"observed_agreement_bps":7500,"expected_agreement_bps":3335,"kappa_millis":624}
```

| κ | 含义 | 下一步 |
|---|---|---|
| ≥ 0.80 | 高度一致，标签可用 | 进入语义评测 |
| 0.60–0.80 | 实质一致，可用 | 可用，但逐条看分歧项 |
| < 0.60 | 条目本身有歧义 | **先修条目，不要修系统** |

`observed_agreement_bps` 是原始一致率，`expected_agreement_bps` 是随机一致率。
**只看原始一致率会高估一致性**，必须看 κ。

## 7. 已知的管道自检

在把这份材料交给真人之前，我用**合成标签**跑通过一遍（两套标签每 4 条故意冲突一条）：

```text
observed 0.7500 / expected 0.3335 / kappa 0.624
```

手算核对：Po = 0.75，Pe ≈ 1/3，κ = (0.75−0.333)/(1−0.333) = 0.625。一致。

**这组数字不是证据**，只证明"导入 → 存储 → κ"这条链路是通的。合成数据已删除。

## 8. 这份标注能回答什么、不能回答什么

能：

- 我们对"该不该追问"的判断，是否至少能被两个独立的人稳定复现；
- 哪些条目本身模糊（`needs_evidence` 集中出现的地方）。

不能：

- 不能证明模型"理解了需求"——它只证明某个具体输出被接受或拒绝；
- 不能替代真实用户。两位标注者是代理，不是被服务的用户；
- 不能给出成本结论。

## 9. 规模与局限

22 条判断 × 2 人，来自 14 个案例，一人撰写。样本小，只能给出方向性结论。
κ 的置信区间会很宽；不要用这批数据做百分比排名。
