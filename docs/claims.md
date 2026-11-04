# 说法与证据对照表

这张表不是成绩单，是**让没证据的话显形**。

规则只有一条：左边每一句我们对外说过的话，右边必须有一条能重跑的证据；
没有就写 `无`，状态标 ❌。❌ 不丢人，❌ 假装成 ✅ 才丢人。

```bash
./scripts/verify-claims.sh          # 只查结构：有没有 ✅ 却没证据
./scripts/verify-claims.sh --run    # 真的去跑每一条证据
```

本表的结构检查已接入 `./scripts/verify.sh`：出现 ✅ 却没有可执行证据时，
全量门禁直接失败。

状态含义：

- ✅ 有可执行的证据，且已经跑过
- ⚠️ 有证据，但只能证明一部分
- ❌ 没有证据；这句话目前不该对外说

证据只允许四种写法，便于机器执行：

```text
test: <测试函数名>        → cargo test --offline <名字>（覆盖 lib 与集成测试）
cmd:  <shell 命令>        → 命令退出码必须为 0
file: <路径>              → 文件必须存在
无                        → 没有证据
```

## 产品承诺

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| 默认不重构：绝大多数请求按原样执行 | 无 | ❌ | held-out 实测 end 误判 6/21，precision 0.778 未达 0.80 门槛 |
| 只问一个最值钱的问题 | cmd: python3 scripts/reframing/analyze_hypothesis_space.py | ⚠️ | 设计层证据（8 个构造案例），不是模型行为证据 |
| 替代方案必须列出新增代价 | 无 | ❌ | 契约字段未实现 |
| 决定权在用户，只读回答不得产生动作回执 | test: read_only_answer_rejects_action_receipts | ✅ | |
| 被拒绝后不再重复提议 | 无 | ❌ | 未实现 |
| 可查询历史决策收据 | 无 | ❌ | `/receipt` 未实现 |

## 工程不变量

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| 无写权限时 execute 模式不调用模型 | test: execute_mode_requires_authorization_without_calling_provider | ✅ | |
| 读文件不能越出允许的读根 | test: rejects_file_outside_allowed_root | ✅ | |
| 搜索不能越出读根，且拒绝符号链接起点 | test: search_rejects_symbolic_link_start_path | ✅ | |
| 工具观测被裁剪，隐藏控制字符被拒绝 | test: bounds_observation_output_and_rejects_hidden_controls | ✅ | |
| 工具调用与结果在 journal 中成对 | test: audit_detects_unpaired_tool_call | ✅ | |
| 远端凭据必须走 HTTPS | test: remote_bearer_credentials_require_https | ✅ | |
| 检查 provider 时不打印密钥 | test: provider_inspection_never_prints_secret_values | ✅ | |
| 写路径重叠的任务被串行化 | test: overlapping_write_roots_are_serialized | ✅ | |
| 平台分支只允许出现在展示适配器 | cmd: ! grep -rnF 'cfg(target_os' src --exclude=tui_terminal.rs | ✅ | 本日修正：原门禁依赖缺失的 rg，被静默跳过 |
| 全量门禁通过（fmt、clippy、测试、评测） | cmd: ./scripts/verify.sh | ⚠️ | 耗时较长，等价于 fmt + clippy + test + eval |

## 发布与复现

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| v0.94.0 源码基线可验证 | file: docs/baseline-2026-11-04.md | ✅ | |
| 发布二进制摘要与文档记录一致 | 无 | ❌ | 文档记 864ffaf0，重建为 dd4faf8e，见基线说明 |
| 跨机器可复现构建 | 无 | ❌ | 只证明了本机重建 |
| Windows 真机运行 | 无 | ❌ | 仅交叉编译 |

## 质量声明

| 说法 | 证据 | 状态 | 说明 |
|---|---|---|---|
| 试点语料覆盖 7 个任务族，且每例可在 2 问内到达动作 | cmd: python3 scripts/reframing/analyze_hypothesis_space.py fixtures/reframing/pilot-v0.json | ✅ | 设计层校验，不是模型行为 |
| 在 11 个构造的 means 案例上，10 个得到 2–3 路问题 | file: docs/evidence/reframing-pilot-v0/summary-v2.json | ⚠️ | 调参集 10/11；held-out 召回 21/21；但 precision 未达标 |
| 真实模型的决策质量 | 无 | ❌ | 无 held-out 结果 |
| 真实语料中"手段类请求"的占比 | 无 | ❌ | 75 条 journal 中真实请求约 10 条，样本不足 |
| 需求重构带来的成本下降 | 无 | ❌ | 未做对照实验 |

## 覆盖率

本表逐条覆盖的条目数，以 `scripts/verify-claims.sh` 的输出为准——不在文档里写死，
避免出现第二个会漂移的数字。README 的 `## Status` 能力清单尚未逐条映射。

**未映射不等于不成立，也不等于成立——只等于没人核对过。**
