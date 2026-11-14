
# 配套设施与平台

[简体中文](ecosystem.md) · [English](ecosystem.en.md)

> 1Agent 是独立产品，不是任何其他设施的附属界面。  
> 下列设施按“可选/独立/研发中”标注，未完成的能力不对外承诺。

## 1. 设施总览

| 设施 | 角色 | 与 1Agent 的关系 | 当前状态 |
|---|---|---|---|
| **1Agent** | 核心 Agent 运行时 | 主体 | **R1 候选**，Linux x86_64 |
| **Skynet Workshop** | 执行底座：Session、沙箱、PTY、租约、Artifact、恢复 | 可选；执行型任务需要 | **研发/联调中**，未作为稳定产品发布 |
| **skyget** | 本地大文件传输工具：断点续传、SHA-256、内容缓存 | 独立工具，不是硬依赖 | **独立可用**，版本以 skyget 自身发布为准 |
| **1A Bot** | 人格化伙伴层：Web/IM/Voice、Companion Contract | 未来消费 1Agent API | **研发中，未发布** |

```text
1A Bot（人格/通道，研发中）
        │
        ▼
1Agent（任务/记忆/技能/授权/审计）  ← 独立产品
        │
        ├─ Skynet Workshop（可选执行底座，研发/联调中）
        └─ skyget（可选大文件传输，独立工具）
```

## 2. Skynet Workshop

### 它是什么

Workshop 是 1Agent 的可选执行底座，负责：

- Session / Pane / Job；
- Linux 沙箱、PTY、进程树、超时；
- 资源租约、容量、写面串行；
- Artifact 存储、路径约束、SHA-256；
- 事件、恢复、接管、验证原语。

### 当前状态

**研发/联调中，不是稳定产品。**

- 已有确定性联调、真实 LLM canary、Artifact 验证、租约/恢复等工程证据；
- W-Gate 正式发布门尚未完全通过；
- 浏览器/computer-use 仍是研究轨道 WS-13；
- 不建议作为生产环境的唯一执行后端；
- 没有 Workshop 时，1Agent 仍可作为**分析/规划型** Super-Agent 使用。

### 如何理解“可执行”

- **无 Workshop**：1Agent 能分析、规划、审计，默认工具只有只读文件能力；
- **接 Workshop**：1Agent 可以把受控任务交给隔离 Session 执行，并独立验收 Artifact；
- “启动成功”不等于“完成”；“exit 0”不等于“verified”。

Workshop 的研发计划见 III-02-Workshop-开发计划；浏览器/子 Agent 的产品化顺序见 III-01-1Agent-Super-Agent-能力补齐计划。

## 3. skyget

### 它是什么

`skyget` 是一个独立的大文件传输工具，适合：

- 大模型、数据集、镜像、构建产物；
- 需要断点续传、后台执行、SHA-256 硬校验的下载/上传；
- 多 Agent/多进程共享同一份内容缓存；
- 不希望把大文件塞进聊天上下文或普通 shell 管道。

### 当前状态

**独立可用，但不是 1Agent 的硬依赖。**

- 当前环境验证版本为 `0.24.x`；
- 有独立的 daemon、CLI 和缓存/ledger；
- 1Agent 的安装包不包含 skyget；
- 小文件和元数据仍可用普通 HTTP 工具处理；
- 大文件传输建议使用 skyget 的 `check` / `get` / `watch` / `pause` / `resume`。

```bash
skyget check <URL> -o <dir>
skyget get <URL> -o <dir> --sha256 <hex> --detach
skyget watch <job-id> --since 0
```

> skyget 的版本、daemon 状态和平台支持以它自己的发布说明为准。  
> 不要把“skyget 可用”理解为“1Agent 已内置浏览器或网络工具”。

## 4. 1A Bot

### 它是什么

1A Bot 是最终的人格化伙伴层：

- Persona / SOUL / 语气；
- Web / IM / Voice；
- Companion Contract；
- Onboarding、隐私和数据页面；
- “一个声音”的对外表达。

### 当前状态

**研发中，未发布。**

- 1Agent 不依赖 Bot 即可运行；
- Bot 不拥有 memory/skill/authority 的权威副本；
- Bot 只能通过 1Agent Surface API 调用；
- 没有 Bot 之前，1Agent 仍可作为独立 Super-Agent 使用；
- Bot 不改变 1Agent 的审计、权限或记忆所有权。

Bot 计划见 III-03-1A-Bot-开发计划；长期演进见 III-03-1A-Bot-长期演进。

## 5. 平台状态

| 平台 | 当前状态 | 说明 |
|---|---|---|
| **Linux x86_64** | **当前正式验证平台** | 发布包、安装脚本、SHA256SUMS、provenance、TUI、CLI |
| **Windows x86_64** | **内部构建，不发布** | 尚未测试；不提供下载，不作为支持平台 |
| **macOS** | **规划中** | 依赖跨平台验证；后续发布 |
| **Android** | **规划中** | 与 Bot/移动伙伴一起推进 |
| **iOS** | **规划中** | 与 Bot/移动伙伴一起推进 |
| **Linux ARM64** | **未发布** | 需要单独构建/验证 |

当前版本是 **Linux 优先**：  
Linux 先稳定；Windows 测试通过后再跟进，macOS 后续；Android/iOS 随 Bot 发布。

## 6. 发布渠道

- **GitHub Releases**：仅 Linux 归档、SHA256SUMS、provenance；Windows 不附带；
- **源码构建**：Rust 工具链 + `cargo build --release`；
- **安装脚本**：包内 `scripts/install.sh`；
- **升级**：`1agent upgrade` / `1agent upgrade --rollback`；
- **数据迁移**：`1agent export` / `restore` / `doctor`。

当前没有自动更新器、系统级服务安装器或移动端安装包。  
这些能力在后续版本中按平台逐步提供。

## 7. 不要误解的地方

- **Workshop 不是 1Agent 的必需组件**；没有它，1Agent 仍然可分析/规划。
- **skyget 不是 1Agent 的浏览器工具**；它只负责大文件传输。
- **Bot 不是 1Agent 的前端壳**；它是未来的人格化伙伴层。
- **Windows 有内部构建产物 ≠ Windows 可下载**；当前未测试，不发布。
- **Android/iOS 没有当前安装包**；不要提供非官方下载。

---

[返回 README](../README.md) · [产品功能](features.md) · [发布状态](status.md)
