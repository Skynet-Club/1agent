# 规划器统一读取入口（read_source）

`read_source` 是 `master plan` 的**类型化读取入口**：模型只判断“要读什么”，
运行时负责“怎么读、能不能读”。模型传入一个 URL 或本地路径，不再自己拼
`lark-cli` 参数，也不需要先学习某个 CLI 的 skill 文档。

## 后端选择

| `source` | 运行时后端 | 说明 |
|---|---|---|
| 飞书/Lark/Doubao 文档 URL（`/docx/`、`/wiki/`） | `lark-cli docs +fetch --doc <url> --doc-format markdown` | 固定只读命令；lark-cli 自己处理 auth |
| 其他 `https://` 公网 URL | `fetch_url` | 拒绝私网/loopback；需要 `--allow-network` |
| 本地文件 | `read_document` | 文本/PDF/Office/图片 OCR 等 |
| 本地目录 / 知识库 | `search_files` | 传 `query` 做搜索；空 query 列目录 |

Obsidian 只是“第三方知识库导出目录”的一个例子；任何目录都走同一套入口。

## 示例

```bash
1agent master plan \
  --task "先读取飞书文档 https://x.feishu.cn/docx/... 和本地知识库，再生成 2 个 Work Item" \
  --source https://x.feishu.cn/docx/... \
  --source /home/skynet/文档/Skynet-Docs/Skynet-Club \
  --config ~/.1a/config.json \
  --workspace /home/skynet/1-agent \
  --allow-network \
  --output plan.json \
  --planner-journal-dir ./journal \
  --json
```

模型侧调用：

```json
{"source": "https://x.feishu.cn/docx/...", "max_chars": 64000}
{"source": "/home/skynet/文档/需求.md"}
{"source": "/home/skynet/文档/Skynet-Docs", "query": "1Agent 发布门禁"}
```

## 安全边界

- 无 shell：Lark 只运行固定的 `docs +fetch` argv，URL 作为单个参数。
- 远程来源需要 `--allow-network`；本地来源只允许在 `read_roots` 内。
- `max_chars` 由运行时收敛到 1,000..200,000；过大的请求不会直接失败。
- 本地文件/目录仍经过 `read_document`/`search_files` 的符号链接、大小、
  目录深度和只读根检查。
- 子进程使用最小环境（HOME/PATH/LANG/代理/证书），不继承 provider key。
- 每次 `read_source` 调用和观察进入 planning journal；来源门禁把成功的
  `read_source` 视为已读。

## 与 `skill_exec` / 来源门禁的关系

- **优先 `read_source`**：它覆盖命名来源的常见读取，模型不需要拼命令。
- `skill_exec` 保留给 `read_source` 覆盖不到的其他只读 skill 命令；仍需
  `~/.1a/skill-exec.json` 白名单和 `--allow-network`。
- 来源门禁仍接受 `read_source`：任务 URL 或 `--source` 声明来源被
  `read_source` 成功读取后，计划不会被 `UnreadSource` 拒绝。

## 计划输出归一化（所有模型）

规划器不按模型名或大小分支；所有 provider 共用同一个输出解析器：

- 接受 `AgentAnswer`（`conclusion` 是字符串或对象）或裸 `PlanProposal`。
- `plan_id` / `goal_id` / `version` 由运行时覆盖，模型只负责内容。
- `expected_artifact` 若是 workspace 下的绝对路径，转成 workspace 相对路径。
- 自审批、Work Item 状态、workspace/read-root 限制、risk 降级、artifact
  规则、schema 和数量上限仍然严格校验。

本地 8B 因此在一次 `read_source` 后产出 3 个有据 Work Item；DeepSeek 无
回归。证据：`docs/evidence/planner-plan-normalize-2026-11-20/`。

## 当前限制

- Lark 适配要求本机安装并登录 `lark-cli`；可用 `LARK_CLI_BIN` 指定路径。
- 只识别 `/docx/`、`/wiki/` 的 Lark 文档 URL；飞书 Base、表格、妙搭页面
  不走 Lark 适配，会落到公网 `fetch_url` 或返回明确错误。
- 目录来源返回的是 `search_files` 的命中/列表，不是整库递归全文；需要具体
  文件时再调用 `read_source`。
- 远程 URL 的 `fetch_url` 只读公网 HTTPS；私网/MCP 来源需要专门的 adapter。
