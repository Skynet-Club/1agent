# ADR 0140: Super-Agent 判断准则 — 四问与四家

- Status: accepted for the prompt-level slice; 12-scenario blind review pending
- Date: 2026-11-16
- Related: ADR-0031, ADR-0039, ADR-0129, `docs/super-agent-doctrine.md`

## Context

1Agent 已有证据、授权、审批、审计和答案契约，但这些规则主要回答“什么不能做”和“做完怎么证明”。它们没有明确回答一个更日常的问题：**面对一件有后果、但并非明显违规的事，应该先做什么、少做什么、由哪种判断力主导？**

固定成某一家学说会坏掉 Super-Agent 的定位：道家容易不作为，墨家容易过度功利，儒家容易人情大于原则，法家容易僵化。产品需要的是“四家都在、因势而主”，而不是四套人格或一个用户可切换的“学派开关”。

## Decision

1. 在完整 Agent 的系统提示词 `provider::agent_system_prompt()` 中加入一个 `SUPER-AGENT DISCIPLINE` 块，位置在基础规则之后、答案 JSON schema 之前。
2. 该块定义四个静默问题：
   - Restraint（知止）：非做不可？最小干预是什么？
   - Benefit（兴利）：对谁有实际好处？成本多少？有没有更有效的办法？
   - Appropriateness（合礼）：对这个用户、关系和时机合适吗？
   - Rule and accountability（明法）：授权够吗？边界在哪？能审计、能回滚吗？
3. `Rule and accountability` 是硬门：不通过就停或问，其他三问没有投票权。通过后，由场景决定主导：目标模糊/过度设计 → 知止；执行/长任务/成本与证据 → 兴利；情绪/关系/高风险人事决定 → 合礼；授权/隐私/不可逆/危机 → 明法。
4. 主导只是透镜，不是人格。任何透镜都不能改变证据、授权、隐私、审计和诚实规则；冲突时基础规则赢。禁止打印四问清单、角色扮演学派、引经据典或把用户称为“学生”。
5. 聊天快路径 `provider::chat_system_prompt()` 不加入该块，保持一次模型调用、无工具的轻量形态。
6. 12 个判断场景写入 `docs/super-agent-doctrine.md`，作为人工盲评协议，不作为自动门禁或发布证据。
7. 不新增用户可切换的“道家/墨家/儒家/法家”模式，不新增四套人格，不改变 Authority/Policy 的 allow/deny 源。

## Consequences

- 完整 Agent 的系统提示词增加约 1.5 KB；聊天快路径的 token 成本不变。
- 四问只在“有后果或歧义的动作”前运行；简单闲聊和简单事实不增加清单。
- 提示词是软约束，不能保证模型每次都不打印清单或不误用透镜；当前只有提示词回归测试，真实行为需要 12 场景盲评。
- 12 场景尚未用真实 provider 跑完；在完成盲评前，不得声称“Super-Agent 判断力已验收”。
- 如果后续需要硬约束（例如某类动作必须走四问），应实现为 Authority/Policy 或运行时前置检查，而不是继续加提示词。
- 0.99 仍未验收；Authority/Policy/Approval 的全链路 metadata 缺口（CQ5/CQ12/CQ16）不受本 ADR 影响。

## Evidence

```bash
cargo test --lib system_prompt_carries_the_super_agent_discipline
cargo test --lib chat_prompt_stays_free_of_the_super_agent_checklist
```

The tests assert the four questions, the hard gate, the lens-not-personality rule, the no-checklist rule, the base-rule precedence, and that the chat fast path remains free of the discipline block. A green test is prompt-contract evidence, not model-quality evidence.
