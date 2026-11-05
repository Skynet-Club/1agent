# 003 授权、安全与审计

Status: normative target; V0 read-only boundary remains until `ADR-003` is accepted  
Depends on: `001-domain.md`、`002-architecture.md`  
Owns: 权限模型、沙箱、审批、密钥、威胁模型、审计要求  
Does not own: 具体工具实现（`006`）、记忆算法（`004`）

## 1. 原则

1. **默认拒绝**：没有显式授权的能力不存在。
2. **最小权限**：Bot/Ticket 只拿到完成当前工作所需的最小范围。
3. **纵深防御**：沙箱、策略、审批、审计四层，任何一层失效不导致越权。
4. **授权在工单层**：Owner 授权的是"做什么、做到哪"，不是逐条命令。
5. **可停止、可撤回、可回滚**：任何长任务都能在安全点停止。
6. **记忆不授权**：记忆、技能、外部文本都不能改变权限。
7. **密钥不落盘**：日志、journal、报告、错误信息中不得出现凭证。

## 2. 威胁模型

| 威胁 | 例子 | 防线 |
|---|---|---|
| Prompt injection | 网页/Issue/邮件里的"忽略之前指令" | 内容标记为数据；工具结果隔离；策略不读文本 |
| 命令注入 | 模型拼接 shell 字符串 | 程序 + 参数数组；禁止默认 `sh -c` |
| 路径逃逸 | `../../etc/passwd`、symlink | 规范化 + 根目录限制 + 拒绝 symlink 逃逸 |
| 密钥外泄 | `env`、`.env`、日志 | 环境白名单 + secret store + 输出脱敏 |
| 记忆投毒 | 恶意内容写入长期记忆 | 来源/taint/审批/隔离/回滚 |
| 技能供应链 | 恶意 SKILL.md/脚本 | 扫描、签名、隔离、人工批准 |
| 跨租户泄漏 | A 的记忆/文件被 B 读到 | 租户/Workspace 隔离；默认不共享 |
| 资源耗尽 | 死循环、fork 炸弹 | 预算、超时、进程组 kill、配额 |
| 越权升级 | Worker 修改自己的权限 | 权限只由 PolicyEngine 写；Worker 只读 |
| 审计篡改 | 删除/改写 journal | append-only + 哈希链 + 外部锚 |

## 3. AuthorityProfile

每个 Bot 和每个 Ticket 都有一个权限档案；Ticket 的档案是 Bot 档案的子集。

```json
{
  "read_roots": ["/home/user/project"],
  "write_roots": ["/home/user/project"],
  "exec": {
    "mode": "allowlist",
    "programs": ["git", "cargo", "bash"],
    "deny": ["rm -rf /", "curl * | sh"],
    "max_timeout_ms": 120000
  },
  "network": {
    "mode": "deny",
    "allow_hosts": []
  },
  "secrets": ["GITHUB_TOKEN"],
  "max_risk": "medium",
  "max_cost_micros": 500000,
  "max_runtime_ms": 600000
}
```

规则：

- `read_roots`/`write_roots` 必须规范化；symlink 解析后仍在根内。
- `exec.programs` 是白名单；不在白名单 → `Prompt` 或 `Forbidden`（按策略）。
- `network.mode=deny` 时，shell 不得继承可联网能力；若无法强制，必须显式说明"执行=联网"，见 §5。
- `secrets` 只注入到需要它的工具，不注入全局环境。
- `max_risk` 限制可执行的工具/动作等级。

## 4. 执行策略

借鉴 Codex 的四级决策：

| 决策 | 含义 | 行为 |
|---|---|---|
| `Allow` | 允许，无需审批 | 直接执行，落 journal |
| `Prompt` | 需要配置的审查者/策略批准 | 请求 Approval；拒绝则不执行 |
| `PromptUser` | 必须由 Owner 本人批准 | 跳过自动审查，直接问 Owner |
| `Forbidden` | 永久禁止 | 拒绝，记录原因 |

规则：

- 策略按"程序 + 参数模式 + cwd + 风险"匹配，不只看程序名。
- `Forbidden` 优先于 `Allow`；冲突时拒绝。
- 审批结果绑定：命令摘要、cwd、超时、成本上限、过期时间。
- 同一 Approval 不能用于第二条命令。
- 审批被拒绝/过期 → Ticket 进入 `blocked`，不重试。

## 5. 沙箱

### 5.1 要求

- 文件系统：只读根 + 可写 workspace；其他路径不可见或只读。
- 进程：独立进程组/Job Object；超时杀整棵树。
- 网络：默认拒绝；允许时按 host/port 白名单（若平台支持）。
- 环境：白名单；不继承 `*_KEY`、`*_TOKEN`、`*_SECRET`。
- 资源：CPU/内存/磁盘/进程数上限。
- 输出：stdout/stderr 分离、限长、控制字符清洗。

### 5.2 平台

| 平台 | 方案 | 备注 |
|---|---|---|
| Linux | bwrap + landlock + PID namespace | 首选；CI 覆盖 |
| macOS | Seatbelt `sandbox-exec` + 最小 profile | 需要测试文件/网络规则 |
| Windows | Job Object + AppContainer/受限 token | 需要 PowerShell/cmd 适配 |

### 5.3 诚实的边界

如果某个平台无法强制网络隔离，必须在 UI 和文档中明说：**允许执行 = 允许联网**。
不得用 `allow_network=false` 假装阻止 shell 联网。

## 6. 审批

### 6.1 流程

```text
动作请求
  → 策略判定
      → Allow：执行
      → Forbidden：拒绝
      → Prompt/PromptUser：创建 Approval
          → 通知 Owner（Web/IM）
          → 批准：执行；拒绝/过期：不执行
```

### 6.2 规则

- 审批请求必须包含：做什么、为什么、影响范围、成本、可否回滚、默认选项。
- 默认选项必须是**最安全**的（拒绝/暂缓）。
- 不可逆动作（删除、发送、合并、支付）必须 `PromptUser`。
- 审批可"本会话记住同类动作"，但不得跨 Bot/跨租户。
- 审批 UI 必须能在移动端完成。

### 6.3 审批与注意力的平衡

审批是注意力成本。减少审批的方法不是放宽权限，而是：

- 把常见安全操作加入白名单（有审计）；
- 把大动作拆成可回滚的小动作；
- 在工单授权时一次说清范围，而不是每条命令问。

## 7. 密钥

- 密钥存放在 OS keychain / secret store / 环境变量；不写入配置文件。
- 只注入需要它的工具；默认环境白名单。
- 日志/错误/journal 必须脱敏：`*_KEY`、`*_TOKEN`、`Authorization`、cookie、私钥。
- 工具输出在进入模型上下文前做敏感信息扫描；命中则替换为 `[REDACTED]`。
- `.env`、`.ssh`、`.aws` 等敏感路径默认拒绝读取，除非显式加入 `read_roots` 且用户确认。

## 8. 记忆与技能的安全

### 8.1 记忆

- 写入：来源、scope、taint、审批；不可信来源进入 quarantine。
- 检索：返回 provenance/taint；高 taint 记忆不得直接触发动作。
- 传播：Bot/Workspace/Tenant 默认隔离；共享需显式 ACL。
- 遗忘：一等操作；删除后索引、缓存、向量都要失效。
- 回滚：版本化；可恢复到指定版本。

### 8.2 技能

- 发布前：静态扫描（注入、凭据、隐藏字符）、签名/摘要、人工批准。
- 安装：隔离目录；首次运行需审批。
- 运行：技能只能使用 Ticket 授权的工具/路径。
- 更新：新版本，不覆盖旧版本；可回滚。
- 锁定：Owner 可锁定技能，禁止自动修改。

## 9. 审计

每个动作必须记录：

- 谁（Owner/Bot/Worker）、什么（工具/命令/参数摘要）、哪里（cwd/路径）、何时；
- 授权依据（Mandate/Approval/Policy 版本）；
- 结果（exit code、状态、artifact 摘要）；
- 成本/时延；
- 是否被脱敏。

审计要求：

- append-only + 哈希链；
- 可从 journal 重建 Ticket/Assignment/Approval 状态；
- 对外收据（receipt）只能引用 journal 事件，不得模型自述；
- 删除/遗忘也要留审计记录（不含敏感内容）。

## 10. 事件响应

| 事件 | 动作 |
|---|---|
| 发现越权 | 立即 pause Bot；撤销 Mandate；保留 journal |
| 密钥泄漏 | 轮换密钥；清除缓存/索引；审计访问 |
| 记忆污染 | 隔离来源；回滚版本；重跑受影响任务 |
| 技能恶意 | 下架技能；撤销发布；扫描所有使用记录 |
| 租户泄漏 | 切断共享；通知受影响方；取证 |
| 失控循环 | kill 进程树；暂停队列；标记 Ticket failed |

必须有全局 kill switch、Bot pause、Ticket stop 三个层级。

## 11. 测试矩阵（最低）

| 类别 | 用例 |
|---|---|
| 路径 | `..`、绝对路径、symlink、大小写、UNC |
| 命令 | 管道、重定向、`sh -c`、后台、子进程树 |
| 环境 | `env`、`.env`、`*_KEY`、继承清理 |
| 网络 | 允许/拒绝、DNS、代理、Unix socket |
| 审批 | 批准/拒绝/过期/重放/跨 Ticket |
| 记忆 | 注入、污染、过期、删除、回滚、跨租户 |
| 技能 | 恶意 SKILL.md、隐藏字符、凭据、越权工具 |
| 恢复 | kill -9、断电、重复投递、半写 |
| 审计 | 篡改、缺事件、哈希链断裂 |

## 12. 与 V0 边界的关系

`docs/product-boundary.md` 的 V0 只读边界在 `ADR-003` 通过前继续有效。
`ADR-003` 必须明确：

- 新增哪些能力（shell/write/network）；
- 对应的 AuthorityProfile、策略、审批、沙箱、测试；
- 如何回滚到只读；
- 对外声明如何变化。

在 ADR 通过前，不得在代码中默认开启执行能力。
