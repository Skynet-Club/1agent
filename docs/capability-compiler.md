# 能力编译器 · Capability Compiler

目标：不要枚举任务类型，也不要把工具名写死。把任务编译成契约，把契约解析成能力，把能力匹配到声明式 manifest，再执行和验收。

## 1. 链路

```text
自然语言任务
→ TaskContract（交付物 + 约束 + 验收 + 歧义）
→ CapabilityRequirement（能力需求）
→ CapabilityResolver（开放 manifest 注册表）
→ selected / alternatives / gaps
→ Tool Plan
→ Execute + Observe
→ Artifact Verify
→ Strategy Memory
```

## 2. TaskContract

```rust
struct TaskContract {
    objective: String,
    deliverable: Option<DeliverableSpec>,
    requirements: Vec<CapabilityRequirement>,
    constraints: Vec<String>,
    acceptance: Vec<String>,
    ambiguities: Vec<String>,
}

struct DeliverableSpec {
    media_type: String,   // open MIME/format, not a task enum
    schema: Option<String>,
    required: Vec<String>,
}

struct CapabilityRequirement {
    verb: String,         // read/render/verify/act/...
    inputs: Vec<String>,
    outputs: Vec<String>,
    constraints: Vec<String>,
    required: bool,
}
```

`media_type` 和 `verb` 是开放词汇，不是任务类型列表。新任务只需要新的 contract，不需要改路由代码。

## 3. Manifest

每个工具/技能/MCP/CLI 声明：

```json
{
  "id": "skill.archify",
  "name": "archify",
  "kind": "skill",
  "source": "hermes optional skills",
  "installed": false,
  "trust": "community",
  "license": "Apache-2.0",
  "capabilities": [
    {
      "verb": "render",
      "inputs": ["structured_spec"],
      "outputs": ["image/svg+xml"],
      "constraints": ["local", "node18"]
    }
  ],
  "verification": ["9-check receipt", "openable artifact"]
}
```

新工具只要加 manifest，就进入候选池；不需要改任务代码。

## 4. Resolver

对每个 requirement：

1. 硬过滤：verb 家族、必需输出、输入交集；
2. 评分：verb、输出、输入、installed、trust、verification；
3. 选择已安装的最高分；未安装候选只作为 alternatives；
4. 没有已安装候选 → 命名 gap，带候选和安装建议。

输出：

```json
{
  "requirements": [
    {
      "requirement": {"verb": "render", "outputs": ["image/svg+xml"]},
      "selected": null,
      "alternatives": [{"name": "archify", "installed": false}],
      "gap": "no installed capability provides render -> [image/svg+xml]"
    }
  ],
  "gaps": ["..."]
}
```

## 5. 工具与 CLI

- 模型可调用 `resolve_capability`：只读，返回 contract、selected、alternatives、gaps。
- 提示词要求：非文本交付物先解析能力；有 gap 时返回 `needs_authorization`/`conditional`，不得静默降级成文字并标 completed。
- 当前 `capabilities/manifests.json` 是种子清单；可继续加工具/技能 manifest。

## 6. P1 runtime enforcement

- `AgentRuntime` runs `capability_preflight` before the model loop for
  non-text deliverable hints. If a required capability has no installed
  candidate, it returns `needs_authorization` and **does not call the model**.
- If a capability resolves, the selected path is added to the run's
  `known_facts`, so the model sees a journaled preflight fact.
- Artifact verifier: `verify_artifact` checks existence, size, MIME, a
  lightweight schema, SHA-256 and an optional loopback preview HTTP 200.
  `CompletionGate` records verified artifacts and can finalize when the step
  budget ends after verification.
- Install proposal: `propose_capability_install` returns a structured proposal
  with the exact command, license, source and `approval_required: true`. The
  CLI separates proposal from approval:

```bash
1agent capability propose-install --name archify --out proposal.json
1agent capability approve-install --proposal proposal.json --yes
```

`approve-install` records the human approver and command hash in
`capability-approvals.jsonl`; it sets `executed: false` and never runs the
install command automatically. A human runs the exact command.

## 7. P2 capability-path memory

- `capability-paths.jsonl` records `requirement → selected manifest → status →
  artifact_verified`, not task-type labels.
- Successful paths add a small explainable boost and can reselect among
  installed candidates; alternatives remain visible.
- `run`/TUI initialize this store at the runtime home; eval does not write it.

## 8. Still open

- Preflight hints are data-driven but only cover declared media hints; an
  entirely unknown artifact format without a hint still needs model/CLI
  `resolve_capability`.
- Install approval records approval but does not execute or verify the install;
  that remains a human operation.
- The six-step headless default is still conservative; complexity-aware
  budgets are future work.
