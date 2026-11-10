# Release Notes — 0.96.0 Action Layer Preview

[简体中文](RELEASE_NOTES.md) · [English](RELEASE_NOTES.en.md)

Date: 2026-11-10  
Status: **R1 public pre-release (release candidate), not production-stable**

GitHub pre-release: <https://github.com/Skynet-Club/1agent/releases/tag/v0.96.0>

## 开发状态与共建

1Agent 仍在快速迭代。0.96.0 是 R1 候选版，不是完成品或生产稳定版；接口、配置和数据格式仍可能变化。

欢迎开发者、测试者、研究者、安全专家、产品与设计伙伴试用 Linux 构建、提交 issue/PR、参与红队和真实工作流验证，并加入 [Skynet-Club](https://skynet.club) 一起完善 1Agent 与开放生态。

## 本版重点

### 1. 从“能回答”到“能动手”

- 本地 `run` / `tui` **默认开启**文件写入和终端执行；`--read-only` 可关闭，`--allow-write` 继续兼容。
- 新增 `write_file` / `edit_file`：允许根校验、拒绝符号链接、覆盖保护、原子替换和内容上限。
- 新增真实 PTY 终端工具 `terminal_create/write/output/exec/destroy`，底层走 Skynet Workshop；无写权限时工具对模型隐藏并由注册表拒绝。Linux x86_64 已验证。
- 新增 `fetch_url`：仅公网 HTTPS，拒绝私网/loopback、逐跳复验重定向、2 MiB 上限和 HTML 正文抽取。
- 新增 `web_search`（Bing RSS，无 key，结果去重、限流如实失败）。
- 新增 `browser_read`：无头 Chrome 渲染公网页面并抽取正文；点击、输入和截图尚未实现。
- 新增 `read_pdf`：通过系统 `pdftotext` 提取 PDF 文本，受读根、符号链接、控制字符和字符上限约束；无 OCR/图片理解。
- 新增 stdio MCP client（`--mcp-config`）：工具发现与调用，远端工具按 `readOnlyHint` 进行写权限门控。
- 新增 `spawn_agent`：有界子进程，独立 journal/工作区/预算，权限继承，深度上限 2。

### 2. 常驻、调度与技能分发

- 新增 `schedule add/list/remove` 和 `daemon`：interval、文件 mtime trigger、持久化、pid 锁、隔离子进程、重启后补跑过期任务。
- 新增 `schedule health`：`ok` / `overdue` / `failing` 三态。
- daemon 在计划任务失败/超时时可写 `schedule_run_failed` 告警日志并发送 webhook；升级阶梯和跨重启去重仍未实现。
- `run/schedule add --routine-skill NAME@VERSION` 只接受已激活的精确技能版本，把技能正文注入任务，不授予额外权限。
- 新增 `skill export/import`：版本化技能包、SHA-256 门禁、manifest 预检、4 MiB 上限和重复版本跳过。
- 新增 Ed25519 技能包签名：`skill keygen`、`skill export --signing-key`、`skill import --public-key`；私钥文件强制 0600，缺公钥、错公钥和篡改均失败。
- 暴露 `skill_list/eval/register/activate/rollback` 与 `delegation_list/register/revoke` 工具。

### 3. 成本、维护与审计

- `cost` 按 provider/model、任务和工具聚合 token、成本、调用/失败次数与 p50/p95 延迟。
- `digest` 汇总最近运行、stop_reason、计划任务、`schedule_health` 和告警计数，不调用模型。
- 新增 `dashboard`：从 journal/schedules/alerts 生成自包含静态 HTML 看板，动态文本转义并带 restrictive CSP，文件权限 0600。
- audit 导出支持 `after_seq` 分页和 `include_records`；默认仍只返回 receipt，避免误导出 journal 内容。
- 新增 `retention`（run journal dry-run/apply + receipt）、`provider-alerts`、`notify`（通用 webhook + Slack/Discord payload）。
- 新增 `memory consolidate`（按 scope + 内容哈希去重并软遗忘旧副本）与上下文图指纹缓存。

### 4. 安全与运行边界

- 工具按权限过滤；写工具在只读运行中对模型隐藏。
- 终端、文件、网络、MCP 与技能工具都有明确的风险等级、超时和失败路径。
- 工具调用与结果在 journal 中成对；模型可见输入可由 journal 重建。
- ADR-0031/0032/0033 分别记录默认执行、PDF 文本提取和技能包/Routine 决策。

## 发布资产

- `1agent-0.96.0-linux-x86_64.tar.gz` — Linux x86_64 包；
- `SHA256SUMS` — 归档与 Linux provenance 校验和；
- `linux-x86_64.json` — Linux 构建 provenance。

Windows 归档不发布；GitHub 生成的源码归档仍可从 tag 获取。

## 升级说明

- 0.96.0 的默认行为是**允许本地写/执行**。如需只读运行，使用 `--read-only`，或在 TUI 输入 `/permissions write off`。
- 技能 CLI 默认遵循 `ONEAGENT_SKILL_JOURNAL`，未设置时使用 `$HOME/.1a/skills.jsonl`。
- `--allow-write` 仍被接受，但与 `--read-only` 互斥；默认已开启，不再需要它。
- 旧 runtime home 的迁移仍通过 `1agent upgrade`；本版没有新增 home 迁移步骤。

## 验证

- 全量门禁：fmt、clippy `-D warnings`、测试、评测与 claims 结构检查全部通过。
- 725 个测试通过，0 失败，5 个 live 测试默认忽略（终端、PDF 等，需要本机后端）。
- claims：81 条检查，62 条 verified，7 条 partial，12 条无证据（未验证的旧产品声明仍在表内显式标注）。
- 发布二进制 `1agent --version` 为 0.96.0；Linux 可复现构建与 provenance 随包提供。
- 实机证据：`docs/evidence/web-2026-11-09/`、`mcp-2026-11-09/`、`subagent-2026-11-09/`、`browser-2026-11-09/`、`terminal-2026-11-09/`、`pdf-2026-11-09/`。

## 已知限制

- 仍不是生产稳定版；A-Gate/S-Gate、外部 IdP、第三方安全审计与 Workshop 完整 release gate 未关闭。
- browser 仅能读取，不能点击/输入/截图；computer-use 未实现。
- PDF 仅文本抽取；扫描件 OCR、图片、语音、DOCX/PPTX 未实现。
- 语义检索/embedding、记忆跨进程轮换未实现。
- 技能 shadow/canary 全链路与真实示范语料未完成。
- Delegation 未接入 Router 全路径；per-tenant 上下文与组织级审计未实现。
- daemon 的 Watchdog 升级阶梯、跨重启去重、真实多日任务证据仍缺。
- 远程技能市场索引、撤销列表和安装 UI 未实现。
- 真实外部 webhook/邮件/日历送达未验证；dashboard 是静态报告而非实时服务。
- Windows 未测试、不发布；macOS、Android、iOS 仍为规划中。

## 许可证

MIT。见 `LICENSE`、`THIRD_PARTY_NOTICES.md` 与 `THIRD_PARTY_LICENSES.md`。
