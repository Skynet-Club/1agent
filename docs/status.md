
# 发布状态与路线图

[简体中文](status.md) · [English](status.en.md)

> 当前公开版本：**0.97.0 R1 release candidate**  
> 当前代码线：**1.0.0-rc.1 code-level candidate（本地，不公开、不是 GA）**；状态与阻塞项见 [status-v1.0.md](status-v1.0.md)  
> 当前平台：**Linux x86_64**  
> 结论：**可以使用和评估，但还不是生产稳定版。**  
> 功能与使用入口：[RELEASE_0.97.md](release/RELEASE_0.97.md) · [功能矩阵](release/FEATURES-0.97.md)

## 0. 还在开发，欢迎共建

- **仍在持续开发**：0.97.0 不是完成品；接口、配置和能力边界仍可能变化。
- **欢迎测试与共建**：欢迎开发者、测试者、研究者、安全专家、产品与设计伙伴试用、提 issue/PR、参与红队和真实工作流验证；加入 [Skynet-Club](https://skynet.club) 一起完善建设。
- **未来是开放的**：浏览器、子 Agent、常驻自主、Workshop、Bot 等能力按公开计划分阶段迭代；路线图可讨论、可修正，不用愿景掩盖当前边界。

## 1. 当前版本是什么

0.97.0 是一个 **R1 release candidate**：

- 不是 1.0；
- 不是生产稳定版；
- 不是“通用 Super-Agent 已全部完成”；
- 不是多租户/组织级产品；
- 不是 Bot。

它已经可以：

- 启动 TUI / headless / 本地服务；
- 完成任务、证据、授权、审批、审计；
- 使用本地记忆、技能、Goal/Commitment、Delegation、ContextGraph；
- 接入 OpenAI-compatible 模型；
- 安装、初始化、导出、恢复、升级、卸载；
- 生成平台专属 Linux 可复现包、SHA256SUMS 与 provenance（Linux 为当前验证平台）。

0.97 的当前证据：100 题闭环 **96/100**；≤1h DeepSeek 任务门 **M1 97.08%**、completed replay **100%**；8h 本地模型运行时 soak **0 崩溃**；独立代码级安全评审 **0 个 P0、5 类 P1 已修**，仍有 4 项 P1 残余（见 `docs/audit-v0.97.md`）。8h DeepSeek 模型质量不作为本版声明。

## 2. 为什么还不是生产稳定版

### 2.1 外部门禁仍未关闭

- A-Gate 正式清单尚未全部勾选；
- S-Gate 未通过；
- 外部 IdP、组织角色、第三方审计/认证未完成；独立代码级评审已完成（0 个 P0、5 类 P1 已修、4 项 P1 残余），不等于第三方认证；
- Workshop 全量 release gate 未完成；
- 跨天/跨周真实长周期任务证据仍缺；
- 系统级安装/卸载和保留期自动执行仍需验收。

### 2.2 能力边界仍明显

- 浏览器只能渲染和读取，不能 computer-use 点击/输入/截图；
- 通用子 Agent、stdio MCP、Web 读取/搜索、本地写/终端已有 MVP，但仍是有界实现；
- daemon/Routine/Trigger 已支持 interval、文件 trigger、重启补跑和技能绑定；任务失败/超时可写告警日志并发 webhook，但升级阶梯和跨重启去重未完成；
- 没有语音/图片理解；`read_pdf` 只抽取 PDF 文本；
- 没有多租户和组织级审计；
- Windows 尚未测试、不发布；macOS 尚未支持；
- 没有 Android/iOS App。

### 2.3 版本本身仍是候选

- 接口、配置和数据格式仍可能变化；
- 真实模型质量依赖 provider 和配置；
- 复杂长任务的成功率仍需更多真实证据；
- 第三方审计、生产签名/审批未完成。

## 3. 平台发布节奏

| 平台 | 状态 | 计划 |
|---|---|---|
| **Linux x86_64** | 当前验证平台 | 先稳定安装/升级/执行/审计链路 |
| **Windows x86_64** | **内部构建，不发布** | 尚未测试；不提供下载，不作为支持平台 |
| **macOS** | 规划中 | Linux 稳定后跟进 |
| **Android / iOS** | 规划中 | 与 Bot/移动伙伴一起发布 |
| **Linux ARM64** | 未发布 | 视需求单独构建/验证 |

> 当前只对 Linux x86_64 的发布包负责。  
> Windows 版本尚未测试，不进入 GitHub Release；内部构建产物只用于后续验证。

## 4. 路线图

### R1：核心运行时收口（当前）

- A-Gate / S-Gate 正式收口；
- 外部 IdP、第三方审计、Workshop release gate；
- 跨天长周期任务；
- 保留期、组织级审计、生产签名/审批；
- Linux 安装/升级/导出/卸载验收。

### R1.5：成本治理（0.97 发布卖点）

- [x] Chat 快速路径：单次调用、无工具、最小 JSON（单次 `max_tokens` 仍待补）
- [x] DirectAnswer 聊天型任务不挂工具，只有明确动作才暴露工具
- [ ] 系统提示词/工具 schema 前缀缓存，动态会话事实后置
- [ ] 工具结果与会话上下文压缩
- [ ] 每次运行显示真实 tokens 与估算成本（本切片未做；不改信息流）
- [x] benchmark 预估、checkpoint、provider 熔断、`--max-cost-micros`
- 目标：简单聊天单题成本降 5–20 倍；工作型任务降 30–50%；失败长跑降 50–80%
- 详见 [cost-governance.md](cost-governance.md)

### R1.6：智能闭环（0.97 发布门禁，AG-23）

- [x] `describe_self`：模型读取实时工具、权限、风险、模型和预算；删除静态能力目录
- [x] `request_tools`：模型可中途请求工具；运行时校验权限并加载 schema
- [x] 证据不足验证/再规划：最多 1–2 次，无死循环
- [x] 模型驱动任务需求评估：freshness/locality/action/risk/ambiguity/budget；关键词只兜底
- [x] 策略记忆：任务特征 → 计划/工具序列/结果；不记忆固定答案
- [x] 100 题评测：自我介绍、最新/本地、追问、简单聊天、高风险动作；M1/M5 不回归
  （最终 v1.3 **96/100**：intro 20/20、追问 20/20、高风险 20/20、简单聊天 19/20、
  最新/本地 17/20；0 hard-gate；completed replay 78/78；3 个联网题耗尽 8 次工具预算
  后返回 `needs_evidence`，1 个简单聊天用了 2 次调用，20 题平均 1.05 次/0 工具。
  人工复核仍未做）
- [x] 8h 本地运行时 soak：Ollama `qwen3-8b-16k`，56 轮 / 448 任务，0 崩溃，
  completed replay 234/234，实际 8h26m51s；本地模型任务质量仅 21.2%，只证明
  运行时稳定性，不作为 M1 模型质量门
- [x] 1h DeepSeek 任务质量门（产品口径限定 ≤1h）：30 轮 / 240 任务，
  M1 233/240 = **97.08%**，suite 23/30 = **76.66%**，completed replay
  239/239 = 100%，crash-free 100%，44m16s，约 ¥11.63；一次 provider error
  丢失了 1 条 case 记录，历史 suite audit+replay 门为 29/30，CLI 显示失败，
  但任务级门通过。8h DeepSeek 模型质量不再作为本次声称
- 未完成不得发布 0.97；详见 Obsidian `III-01-1Agent-开发计划` 4.4/4.5

### R2：行动层（部分已落地）

- [x] `fetch_url` / `web_search` 只读网页获取；
- [x] `write_file` / `edit_file` / 真实 PTY 终端（写权限与 Authority/Policy 约束）；
- [x] stdio MCP 客户端；
- [x] 有界子 Agent（独立 journal/工作区，深度 2）；
- [x] `browser_read` 无头 Chrome 渲染；
- [ ] Workshop computer-use（点击/输入/截图，WS-13）；
- [ ] HTTP/SSE MCP 与工具市场。

### R3：Standalone Super-Agent

- 跨天/跨周任务；
- Goal/Commitment 依赖与证据；
- Attention 全量；
- 技能捕获→评测→激活→回滚闭环；
- 自我总结/进化闭环；
- 有界子 Agent 映射 Workshop Worker；
- S-Gate 通过。

### G2 / R4+

- daemon / Routine / Trigger / Source / Watchdog；
- 自动进化、技能库；
- 多 Bot、多租户、组织级审计；
- 移动端与生态。

> 路线图是方向，不是日期承诺；每次范围变更走计划索引和治理记录。

## 5. 研发中能力清单

| 能力 | 状态 | 预计阶段 |
|---|---|---|
| Skynet Workshop 稳定执行 | 研发/联调中 | R1/R2 |
| 浏览器 / computer-use | 读取部分可用；操作未实现 | R2 |
| 通用子 Agent | 有界 MVP 可用 | R2/R3 |
| MCP 客户端 | stdio 部分可用 | R2 |
| 常驻 daemon/Routine | interval/trigger/重启补跑/health 可用；自动 Watchdog 未实现 | G2 |
| 全量 Attention/Digest | Digest 可用；维护循环未实现 | G2 |
| 自动进化/技能市场 | 审批/回滚 + SHA-256 技能包；签名/远程市场未实现 | G2/G3 |
| 多租户/组织级审计 | 未实现 | R6 |
| Bot（人格/通道） | 研发中 | R4+ |
| Windows 正式支持 | 内部构建，未测试，不发布 | 后续 |
| macOS | 规划中 | 后续 |
| Android/iOS | 规划中 | Bot 阶段 |

## 6. 发布包里有什么

对外 GitHub Release 只提供 Linux 包；Windows 包不发布。Linux 发布包包含：

- `1agent` 二进制；
- `scripts/install.sh`；
- `LICENSE`、`LICENSES/`；
- `SECURITY.md`、`PRIVACY.md`；
- `THIRD_PARTY_NOTICES.md`、`THIRD_PARTY_LICENSES.md`；
- `docs/`、`evals/`、`configs/`；
- `SHA256SUMS`；
- SHA256SUMS 与 provenance 文件。

验证：

```bash
1agent --version
1agent doctor
sha256sum -c SHA256SUMS
```

> 当前双平台 release manifest 仅用于内部验证；公开预发布使用平台专属 Linux archive、SHA256SUMS 和 Linux provenance，不附带 Windows 资产。

## 7. 对外表述边界

可以说：

- “1Agent 是一个本地优先、可审计的 Agent 运行时”；
- “当前版本是 R1 release candidate”；
- “Linux x86_64 是当前验证平台”；
- “可选 Workshop 执行，尚未作为稳定产品发布”；
- “浏览器读取、子 Agent、stdio MCP、常驻调度已有有界 MVP；computer-use 和自动 Watchdog 仍未完成”。

不要说：

- “生产稳定版”；
- “通用 Super-Agent 已全部完成”；
- “可操作任意网站”；
- “Windows/macOS/Android/iOS 已支持”；
- “多租户/组织级审计已可用”；
- “通过第三方安全审计”。

---

[返回 README](../README.md) · [设计哲学](design-philosophy.md) · [配套设施](ecosystem.md) · [Super-Agent 能力补齐计划](status.md)
