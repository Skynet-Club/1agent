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

## 6. 尚未完成

- P1：运行时强制 preflight（不是只靠提示词），artifact verifier，安装提案与人工批准。
- P2：按能力路径学习，而不是按任务类型学习。
- 当前 `resolve_capability` 只读，不会安装或执行候选。
