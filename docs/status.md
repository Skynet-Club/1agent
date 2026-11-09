
# 发布状态与路线图

[简体中文](status.md) · [English](status.en.md)

> 当前版本：**0.95.0 R1 release candidate**  
> 当前平台：**Linux x86_64**  
> 结论：**可以使用和评估，但还不是生产稳定版。**

## 0. 还在开发，欢迎共建

- **仍在持续开发**：0.95.0 不是完成品；接口、配置和能力边界仍可能变化。
- **欢迎测试与共建**：欢迎开发者、测试者、研究者、安全专家、产品与设计伙伴试用、提 issue/PR、参与红队和真实工作流验证；加入 [Skynet-Club](https://skynet.club) 一起完善建设。
- **未来是开放的**：浏览器、子 Agent、常驻自主、Workshop、Bot 等能力按公开计划分阶段迭代；路线图可讨论、可修正，不用愿景掩盖当前边界。

## 1. 当前版本是什么

0.95.0 是一个 **R1 release candidate**：

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

## 2. 为什么还不是生产稳定版

### 2.1 外部门禁仍未关闭

- A-Gate 正式清单尚未全部勾选；
- S-Gate 未通过；
- 外部 IdP、组织角色、第三方审计未完成；
- Workshop 全量 release gate 未完成；
- 跨天/跨周真实长周期任务证据仍缺；
- 系统级安装/卸载和保留期自动执行仍需验收。

### 2.2 能力边界仍明显

- 没有浏览器 / computer-use；
- 没有通用子 Agent；
- 没有 MCP 客户端；
- 没有完整 daemon/Routine/Trigger/Watchdog；
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

### R2：行动层

- `web_fetch` 只读网页获取；
- shell/write/edit/git/test 工具矩阵（经授权/沙箱）；
- MCP/连接器；
- 逻辑子 Agent（只读研究/对比/验证）；
- Workshop 浏览器/computer-use 产品化（WS-13）。

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
| 浏览器 / computer-use | 未实现 | R2 |
| 通用子 Agent | 未实现 | R2/R3 |
| MCP 客户端 | 未实现 | R2 |
| 完整常驻 daemon/Routine | 未实现 | G2 |
| 全量 Attention/Digest | 研发中 | G2 |
| 自动进化/技能市场 | 研发中/规划 | G2/G3 |
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
- “浏览器、子 Agent、常驻自主仍在研发”。

不要说：

- “生产稳定版”；
- “通用 Super-Agent 已全部完成”；
- “可操作任意网站”；
- “Windows/macOS/Android/iOS 已支持”；
- “多租户/组织级审计已可用”；
- “通过第三方安全审计”。

---

[返回 README](../README.md) · [设计哲学](design-philosophy.md) · [配套设施](ecosystem.md) · [Super-Agent 能力补齐计划](status.md)
