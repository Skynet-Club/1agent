# 规划器技能执行（skill_exec）

`skill_exec` 让 `master plan` 在规划前调用外部 skill CLI 的**只读**命令，
例如用 `lark-cli` 读取飞书文档。它不是通用 shell：命令必须由操作者在
`~/.1a/skill-exec.json` 中显式声明，且只匹配声明过的参数前缀。

## 配置

默认路径 `~/.1a/skill-exec.json`；`ONEAGENT_SKILL_EXEC=/path/to/file.json`
可覆盖。文件不存在时能力关闭。

```json
{
  "schema_version": 1,
  "commands": [
    {
      "name": "lark-cli",
      "bin": "/home/skynet/.local/bin/lark-cli",
      "read": [
        ["skills", "list"],
        ["skills", "read"],
        ["docs", "+fetch"],
        ["docs", "+search"]
      ]
    }
  ]
}
```

- `name`：模型调用时使用的名字，必须唯一。
- `bin`：可执行文件。绝对路径，或在 `PATH` 中可解析的名字。
- `read`：允许的参数前缀列表。模型只能在某个前缀后追加参数，不能替换前缀。
  例如 `["docs", "+fetch"]` 允许 `docs +fetch --doc <url>`，但拒绝
  `docs +create`。

## 规划器调用

```bash
1agent master plan \
  --task "先读飞书文档 https://example.feishu.cn/docx/xxxx，再按文档要求规划改动" \
  --config ~/.1a/config.json \
  --workspace /path/to/workspace \
  --allow-network \
  --output plan.json \
  --planner-journal-dir ./journal \
  --json
```

流程示例：

1. `skill_exec {"skill":"lark-cli","args":["skills","read","lark-doc"]}`
2. `skill_exec {"skill":"lark-cli","args":["docs","+fetch","--doc","<url>","--doc-format","markdown"]}`
3. 结合本地 `read_file`/`search_files` 生成 Work Item。

只有在同时满足以下条件时，规划器才会看到 `skill_exec`：

- 传入 `--allow-network`；
- `~/.1a/skill-exec.json` 存在且有效。

## 安全边界

- **无 shell**：参数按 argv 直接传给可执行文件，`;`、`$()` 等不会被解释。
- **只读白名单**：只运行配置中声明的前缀；写命令和未声明命令在工具层被拒绝。
- **联网授权**：工具要求 `PermissionSet.allow_network`，由 `--allow-network` 打开。
- **最小环境**：子进程只继承 `HOME`、`PATH`、`LANG`、`TZ`、`TMPDIR`、
  代理和证书相关变量，不继承 provider API key 等其余环境变量。
- **有界**：单次 90 秒；stdout 最多 64k 字符，stderr 最多 8k 字符；
  规划器全局仍受 12 步 / 24 次调用 / 200k 字符限制。
- **审计**：每次 `skill_exec` 调用、参数和观察结果都写入 planning journal。
- **只读的边界**：只读指被调用的命令前缀；CLI 自己可能刷新 token 缓存，
  但不应修改用户文档。配置写命令前缀等于放弃这层保证。

## 与 `read_document` / `skill_search` 的区别

| 工具 | 作用 | 是否执行外部程序 |
|---|---|---|
| `read_document` | 读本地文本/PDF/Office/图片等 | 是，调用本地转换器（pdftotext 等） |
| `skill_search` | 发现已安装 skill，返回 metadata + body | 否 |
| `skill_exec` | 运行操作者声明的只读 skill CLI 命令 | 是，按白名单 |

`skill_search` 不扫描 CLI 内嵌的 skill（例如 `lark-cli` 的 `lark-doc`）；
需要时由 `skill_exec` 调用 `lark-cli skills list/read` 发现和阅读。

## 来源门禁

**解决什么**：确定性计划校验只看结构、ID 和权限，看不出模型有没有真的读
来源。门禁把“结构合法但没读来源”的计划变成明确的 `UnreadSource` 错误，宁可
不给计划，也不给假计划。

**来源怎么声明**：

| 来源 | 声明方式 | 通过条件 |
|---|---|---|
| 任务里的 `http(s)` URL | 自动识别 | 某次**成功**的 `skill_exec` 调用参数包含该 URL |
| 本地文件 | `--source <文件>` | `read_file` 或 `read_document` 成功读过该文件 |
| 本地目录 / 知识库 | `--source <目录>` | 目录下有过成功的 `read_file` / `read_document` / `search_files` |
| 显式 URL | `--source <url>` | 同 URL |

`--source` 可重复。本地来源同时成为规划器的读取根：文件授权其父目录，目录
授权自身。Work Item 仍限制在 `context.workspace` 内；外部来源由规划器读取，
再把关键事实写进 objective。这样不会顺手放大 Worker 的读取权限。

Obsidian 只是“第三方知识库入口”的一个例子；任何导出成目录的笔记/知识库都
走同一套 `--source` + 现有读取工具，不新增 Obsidian 专用工具。

匹配 URL 时忽略 `#fragment`、`?query` 和末尾 `/`。门禁只保证“读过”，不保证
“读懂了”；模型仍可能在读过之后再编。

## 已验证

2026-11-20：DeepSeek 通过 `lark-cli docs +fetch` 读取飞书 DOCX
（`document_id=HztBdLEAXorDhQxCYp1cyDXOnmd`，revision 8），并产出 3 个
Work Item；一次 `--doc-url` 错误被当作观察返回，规划器自行改用 `--doc`。
同一版本还验证了本地来源：`--source /tmp/.../requirements.md` 被
`read_file` 成功读取后产出 2 个 Work Item；本地 `qwen3-8b-16k` 未读 URL
时被 `UnreadSource` 拒绝。证据：
`docs/evidence/planner-skill-exec-2026-11-20/`、
`docs/evidence/planner-source-gate-2026-11-20/`。

### 跨模型（2026-11-20）

同一任务、同一策略、同一安装二进制：DeepSeek `deepseek-flash` 10/10
（读到 revision 8，产出 3 个有据 Work Item）；本地 `qwen3-8b-16k` 1/10
（未调用 `skills list/read`，5 次工具调用均失败或无效，最后凭空生成
多 cloud/SOC2/Grafana 计划）。机制两边都可用，但小模型不会按工具错误
自纠，确定性计划校验也抓不住“没读文档”。证据：
`docs/evidence/planner-skill-exec-cross-model-2026-11-20/`。

## 当前限制

- 只接入 `master plan` CLI；TUI 的 production planner 仍是无工具单轮。
- 配置错误（JSON 非法、schema 不支持、命令名重复、前缀不安全）会让
  `master plan --allow-network` 直接失败，而不是静默关闭。
- 目前没有按命令声明“是否需要联网”；所有 `skill_exec` 都需要 `--allow-network`。
