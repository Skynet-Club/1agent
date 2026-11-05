# 1A Bot 开发手册

Status: draft for review · 2026-11-06  
Owner: 1A 产品与架构  
Audience: 架构、内核、Web、运行时、评测、安全、文档

这本手册把"1A 是一个超级智能伙伴"拆成可执行的工作包。它不是愿景文档；每一条
`MUST` 都可以被实现、测试或审计。

## 三层定位（先读这一段，后面所有章节服从它）

| 层 | 代号 | 职责 | 不是 |
|---|---|---|---|
| 内核 | 1A Kernel | 事件、工单、授权、执行、记忆、审计 | 不是聊天框 |
| 基础设施 | 1A + Workshop | 调度、执行、验收、恢复 | 不是产品 |
| 产品 | 1A Bot | 长期在线的超级智能伙伴，Web 为主界面 | 不是 copilot，不是一次性会话 |

**当前阶段**：基础设施成熟期。Bot Alpha 只有在 [009-roadmap.md](009-roadmap.md) 的
`G0` 门禁全部通过后才能启动。

## 阅读顺序

```text
README（本文件）
  → 000 命题与不变量
  → 001 领域模型与术语
  → 002 架构与边界
  → 003 授权、安全与审计
  → 004 记忆系统
  → 005 技能与自我进化
  → 006 运行时
  → 007 界面与交互
  → 008 质量与评测
  → 009 路线图与门禁
  → 010 治理、决策与一致性
```

`000`–`003` 是**规范性**的：任何实现、PR、设计稿都不得违反。  
`004`–`008` 是**设计规范**：可以迭代，但必须先改文档再改代码。  
`009`–`010` 是**执行与治理**：工作包、门禁、决策记录。

## 规范性关键词

- **MUST / 必须**：不满足即不能合并、不能发布。
- **SHOULD / 应当**：默认这样做；偏离必须在 PR 或 ADR 中写明理由。
- **MAY / 可以**：可选，不构成承诺。

## 单一事实来源

每个概念只允许有一个"所有者文档"。重复定义视为缺陷。

| 内容 | 所有者 |
|---|---|
| 产品定位、不变量、非目标 | `000-thesis.md` |
| 术语、实体、状态机 | `001-domain.md` |
| 分层、组件、依赖方向 | `002-architecture.md` |
| 权限、沙箱、审批、密钥、威胁模型 | `003-authority.md` |
| 记忆分层、写入、检索、遗忘、用户模型 | `004-memory.md` |
| 技能格式、进化闭环、评测门禁 | `005-evolution.md` |
| 事件循环、Router/Worker、恢复 | `006-runtime.md` |
| Web/TUI/IM/语音界面 | `007-surfaces.md` |
| 测试、基准、发布门禁 | `008-quality.md` |
| 阶段、工作包、退出条件 | `009-roadmap.md` |
| ADR、决策日志、开放问题、一致性检查 | `010-governance.md` |

已有文档继续有效，但只作为**实现细节**，不得与手册冲突：

- `docs/product-boundary.md`：V0 边界；Bot Alpha 的授权模型未通过 `ADR-003` 前仍然有效。
- `docs/self-evolution.md`：L2 经验生命周期；手册在其上扩展，不重新定义。
- `docs/master-agent.md`：Work Item / lease / 验收；映射见 `001-domain.md`。
- `docs/architecture.md`、`docs/io-contracts.md`：内核契约。

## 工作方式

每个工作包必须按仓库的飞轮执行：

```text
需求 → 设计（必要时 ADR） → 实现 → 测试 → 验证 → 审计 → 文档 → 下一个需求
```

提交前必须跑：

```bash
./scripts/verify.sh
```

涉及外部声明时，还必须更新 `docs/claims.md` 并给出可重跑证据。

## 当前基础（不要重复造）

已经存在、应当复用：

- append-only journal、replay、audit、checkpoint/resume；
- provider 路由、重试、成本账本、上下文压缩；
- requirement regression（`/reframe`）、claims/evidence 契约；
- experience store：draft → reviewed → verified → approved → active → degraded/rolled_back；
- Master Work Item、lease、idempotency、独立验收；
- TUI、命令面板、主题、PTY 测试；
- 367 个确定性测试（截至 2026-11-06）。

尚未存在、不得假装存在：

- shell / 写文件 / 网络工具；
- Bot 身份、跨会话记忆、Web UI、消息网关；
- 多 Bot 团队、技能市场、常驻服务；
- Windows/macOS 的沙箱与 CI 矩阵。

## 变更协议

1. 先改所有者文档，再改代码；代码 PR 必须链接对应章节。
2. 新增术语先加到 `001-domain.md` 的术语表；未登记的术语不得进入 API。
3. 任何与不变量冲突的需求，先开 ADR，不得直接实现。
4. 每个阶段结束更新 `009-roadmap.md` 的门禁状态与 `010-governance.md` 的决策日志。
5. 删除或重命名概念必须走弃用流程，见 `010-governance.md`。

## 目录

- [000 命题与不变量](000-thesis.md)
- [001 领域模型与术语](001-domain.md)
- [002 架构与边界](002-architecture.md)
- [003 授权、安全与审计](003-authority.md)
- [004 记忆系统](004-memory.md)
- [005 技能与自我进化](005-evolution.md)
- [006 运行时](006-runtime.md)
- [007 界面与交互](007-surfaces.md)
- [008 质量与评测](008-quality.md)
- [009 路线图与门禁](009-roadmap.md)
- [010 治理、决策与一致性](010-governance.md)
