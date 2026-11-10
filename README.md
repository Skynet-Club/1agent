
# 1Agent

[简体中文](README.md) · [English](README.en.md)

> **先问一句，再动手。**

**本地优先、可审计、可解释的 Super-Agent 运行时。**

[![Status: R1 release candidate](https://img.shields.io/badge/status-R1%20release%20candidate-orange)](#current-status)
[![Platform: Linux x86_64](https://img.shields.io/badge/platform-Linux%20x86__64-blue)](docs/ecosystem.md)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](https://opensource.org/licenses/MIT)
[![Rust 2024](https://img.shields.io/badge/rust-2024-orange)](https://www.rust-lang.org/)

1Agent 不是“接一个大模型就能聊天”的壳。它把**任务、证据、授权、执行、验收、审计**做成一条可回放的链路：  
它先说清楚为什么做，再决定做什么；能分析；接上受控执行底座后，也能执行；每一步都留下可验证的记录。

大多数 Agent 会直接给你一份“电钻推荐清单”。  
1Agent 会先问：你打孔，是为了穿线，还是为了挂东西？

> **项目仍在持续开发。** 当前版本 **0.96.0**（2026-11-10 更新），R1 release candidate，不是完成品；欢迎测试、反馈和共建。  
> 本版重点：本地写/终端默认开启；文件、Terminal、Web、PDF、MCP、子 Agent；调度/Routine；签名技能包；成本/审计/dashboard。  
> 加入 [Skynet-Club](https://skynet.club)，一起完善 1Agent 与开放生态。  
> 当前 Linux 预发布：[v0.96.0](https://github.com/Skynet-Club/1agent/releases/tag/v0.96.0) · [发布说明](docs/release/RELEASE_NOTES.md)。

---

## 为什么是 1Agent

| 原则 | 具体做法 |
|---|---|
| **先问目的** | 区分“目标”与“手段”；必要时只问一个能改变方案的问题 |
| **证据优先** | journal、receipt、replay 是产品原语；没有 journal 的“完成”不算完成 |
| **人在回路** | 本地 CLI/TUI 默认允许文件写入和终端执行；`--read-only` 可关闭；网络需 `--allow-network`；不可逆动作逐次确认 |
| **本地优先** | 单一 runtime home（`$HOME/.1a`）；数据可导出、可恢复；不要求云账户 |
| **Provider 中立** | 确定性 provider、OpenAI-compatible provider；模型可替换，路由可解释 |
| **可逆** | 记忆软/硬遗忘、技能回滚、进化回滚；变更必须能撤回 |
| **简单优先** | Occam 输出契约：先给结论，再给证据；不堆砌无关分析 |
| **一个声音** | Worker 不对用户说话；由 1Agent 统一组装最终答案 |

更完整的设计取舍见 [DESIGN.md](docs/design-philosophy.md)。

---

## 现在能做什么

- **交互式 TUI**：对话模式、`/` 命令抽屉、分类与最近命令、上下文滚动、复制、运行状态。
- **Headless 运行**：`1agent run`，适合脚本、CI 和批处理。
- **证据化任务**：任务图、Ticket、Mandate、审批、SideEffect、Report、Receipt。
- **本地记忆**：四层记忆、scope/taint/TTL、加密 payload、软/硬遗忘。
- **技能生命周期**：SKILL.md、版本、扫描、静态评测、运行时 canary、回滚。
- **长期目标**：Goal/Commitment、确定性 Attention 基础、个人上下文视图、DelegationProfile。
- **自我总结与进化**：Reflection/Lesson、EvolutionProposal、Ledger、人工审批与回滚。
- **模型路由**：确定性 provider、OpenAI-compatible provider、成本/延迟账本、影子执行。
- **审计**：哈希链 journal、replay、receipt、查询与导出。
- **执行底座**：接入 Skynet Workshop 后，可在受控 Session/沙箱中执行并验收 Artifact；终端工具依赖该底座。
- **工具面**：`read_file` / `read_pdf` / `search_files`、`write_file` / `edit_file`、真实 PTY 终端、`fetch_url` / `web_search` / `browser_read`、stdio MCP、`spawn_agent`。
- **常驻与技能分发**：`schedule` / `daemon` / `schedule health`、`--routine-skill`、SHA-256 门禁的 `skill export/import`。
- **运行看板**：`cost`（provider/model/任务/工具）、`digest`（运行/计划/健康/告警）、`dashboard`（静态 HTML）。

当前**没有** computer-use 点击/截图、语音/图片理解、自动 Watchdog 升级策略、移动端和 Bot。


这些不是“隐藏功能”，而是明确写在 [STATUS.md](docs/status.md) 里的研发中/未实现项。

---

## 快速开始（Linux x86_64）

> 当前正式验证平台只有 **Linux x86_64**。Windows 版本尚未测试，**暂不发布、不提供下载**；macOS、Android、iOS 后续发布。

从 Releases 下载 Linux 包后：

```bash
tar -xzf 1agent-0.96.0-linux-x86_64.tar.gz
cd 1agent-0.96.0
./scripts/install.sh --prefix "$HOME/.local"
export PATH="$HOME/.local/bin:$PATH"

1agent --version
1agent init
1agent doctor
```

启动离线 TUI（确定性 provider，不需要网络）：

```bash
1agent tui
```

接入真实模型（示例为 DeepSeek-compatible endpoint；密钥只放环境变量）：

```bash
export DEEPSEEK_API_KEY="..."
1agent tui --config configs/example.json --allow-network
```

Headless 运行：

```bash
1agent run --task "比较方案 A 和方案 B，并给出翻转条件"
1agent run --task "..." --config configs/example.json --allow-network
```

完整的安装、provider、TUI、导出/恢复、升级和故障排查见 [MANUAL.md](docs/manual.md)。

---

## Current status

**R1 release candidate。不是 1.0，不是生产稳定版。**

| 能力 | 状态 | 说明 |
|---|---|---|
| 1Agent 核心运行时 | **R1 候选** | TUI/CLI、任务、授权、记忆、技能、审计、打包 |
| Linux x86_64 | **当前验证平台** | 发布包、安装脚本、SHA256SUMS、provenance |
| Windows x86_64 | **不发布（未测试）** | 仅内部构建；不提供下载，不作为支持平台 |
| macOS / Android / iOS | **规划中** | 后续跟进；移动端与 Bot 一起推进 |
| Skynet Workshop 执行 | **研发/联调中** | 可选执行底座；W-Gate 未正式通过 |
| Web 读取/搜索 | **部分** | `fetch_url` + `web_search`（Bing RSS，尽力而为）；登录态网页未实现 |
| 文档读取 | **部分** | `read_pdf` 调用系统 `pdftotext` 提取文本；图片/扫描件 OCR、DOCX/PPTX 未实现 |
| 浏览器 / computer-use | **部分** | `browser_read` 无头 Chrome 渲染；点击/输入/截图/computer-use 未实现 |
| 通用子 Agent | **部分** | `spawn_agent` 有界子进程（深度 2、独立 journal/工作区）；Master→Worker 调度仍独立 |
| 常驻 daemon / Routine | **部分** | `schedule` + `daemon`（interval、文件 trigger、持久化、pid 锁、重启补跑）；`--routine-skill`；`schedule health`；失败/超时告警可写日志并发 webhook；升级阶梯和跨重启去重未实现 |
| 成本/延迟看板 | **部分** | `cost` 按 provider/model/任务聚合并统计工具调用/失败；`dashboard` 生成静态 HTML 看板；工具级 token 归因未实现 |
| 保留期/删除 | **部分** | `retention` 可 dry-run/apply 删除过期 run journal 并写 receipt；memory/audit 保留期未覆盖 |
| 记忆整合 | **部分** | `memory consolidate` 重复内容软遗忘；语义检索/embedding 未实现 |
| 通知渠道 | **部分** | `notify` 通用 HTTPS/loopback webhook，支持 slack/discord payload；邮件/日历连接器与真实外部送达未验证 |
| 技能运行时 | **部分** | `skill_list/eval/register/activate/rollback` 工具已暴露；`skill export/import` 支持 SHA-256 门禁与 Ed25519 签名；真实示范语料、shadow/canary、远程市场与撤销列表未完成 |
| 委托运行时 | **部分** | `delegation_list/register/revoke` 工具已暴露并持久化；Router 全路径委托接线未完成 |
| 1A Bot | **研发中** | 人格化伙伴层；1Agent 不依赖 Bot |
| MCP / 工具连接器 | **部分** | stdio MCP client + `--mcp-config`；HTTP/SSE 与工具市场未实现 |

完整状态见 [STATUS.md](docs/status.md)。

> 终端执行工具（`terminal_*`）当前只随 Linux x86_64 验证；macOS/Windows 需要等待 Skynet Workshop 的 daemon/GUI 与 ConPTY 支持，届时 1Agent 的工具契约不变。

---

## 配套设施

| 设施 | 角色 | 当前状态 |
|---|---|---|
| **Skynet Workshop** | 可选执行底座：Session、沙箱、租约、Artifact、恢复 | 研发/联调中，尚未作为稳定产品发布 |
| **skyget** | 独立的大文件传输工具：断点续传、SHA-256、内容缓存 | 独立可用；不是 1Agent 的硬依赖 |
| **1A Bot** | 人格化伙伴层：Web/IM/Voice、Companion Contract | 研发中，未发布 |

详见 [ECOSYSTEM.md](docs/ecosystem.md)。

---

## 文档

- [设计哲学](docs/design-philosophy.md) — 为什么先问目的、为什么证据优先
- [产品功能](docs/features.md) — 当前能力、边界和明确未实现项
- [操作手册](docs/manual.md) — 安装、初始化、provider、TUI、运行、导出/恢复
- [配套设施](docs/ecosystem.md) — Workshop、skyget、Bot 与平台状态
- [发布状态与路线图](docs/status.md) — 当前版本、平台计划、研发中能力

---

## 参与建设

1Agent 还在持续开发。我们欢迎各路开发者、测试者、研究者、安全专家、产品与设计伙伴参与：

- **试用与反馈**：下载 Linux 包，报告安装、TUI、任务、记忆、技能、审计问题；
- **贡献代码与文档**：提交 issue / PR，补充测试、Provider、工具、场景和文档；
- **安全与评测**：参与红队、隔离测试、长周期任务和真实工作流验证；
- **加入社区**：加入 [Skynet-Club](https://skynet.club)，一起完善产品、计划和生态。

我们的未来构想是开放的：按公开计划分阶段迭代，每个阶段用测试、证据和真实反馈推进；路线图可以讨论、可以修正，不把愿景当成一次性承诺。

---

## 安全与隐私

- 本地 CLI/TUI 默认允许文件写入和终端执行；`--read-only` 可关闭；网络和外部副作用仍需显式授权。
- 密钥通过环境变量或本地密钥文件引用，不写入 journal、报告或配置。
- 导出包不包含密钥材料；恢复和迁移有 SHA-256 校验。
- 详细边界见仓库 `SECURITY.md`、`PRIVACY.md` 和 [DESIGN.md](docs/design-philosophy.md)。

---

## License

[MIT](https://opensource.org/licenses/MIT)。第三方 notices 与许可证清单随发布包提供。
