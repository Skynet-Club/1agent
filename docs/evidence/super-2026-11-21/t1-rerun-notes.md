# 任务笔记

目标：读飞书 wiki《加入我们微页面文案》→ 深读后做成手机端可交互单页 → unslop 质检 → `/tmp/1agent-t1-final3/index.html`。

## 约束
- 不改 `/home/skynet/work/join-us`（只读参考其 REVIEW.md 与 source.md）。
- 单文件 HTML；无外部请求；触控 ≥44px；无横向溢出；无占位内容。

## 决定
- 源内容：`/home/skynet/work/join-us/source.md`（lark-cli 输出的 JSON，document_id Higbd259domECKx3XuZcinYWnEg，revision 990）。本轮无法重新拉取（见下）。
- 终端不可用：`terminal_create` 连续 2 次返回 `session_limit：活动会话最多 128 个`，因此 lark-cli 不能重跑、二进制图片不能复制/编码。
- 图片：不使用飞书图片或本地 img/ 拷贝（需 shell），首屏与分节视觉全部用内联 SVG 手绘（unslop 允许 code-built visual），不伪造二维码。
- `ui-ux-pro-max`：本机未安装（catalog 仅候选；propose_capability_install 报 "no capability manifest"），按 preflight 用 manual-html + 自建设计系统，并记录缺口，不假装用过。
- `unslop`：直接读 `~/.hermes/skills/creative/unslop/SKILL.md`（32057 B）按其九遍 pass 与清单自查。

## 方向（unslop 五句）
- 产品：宋庄晨启教育（智育颂®悠学城·素养融合空间）的招聘微页面，招校区教学岗与运营岗。
- 性格：温厚、具体、手作感。
- 世界：红砖临街店、白桌子工作室、五分钟外的花田、宣纸。
- 时刻：唯一的富色彩元素 = 首屏底部的花田地平线（手绘 SVG）。
- 记号：小标题下一条细地平线 + 一个花点，只在分节标题出现。

## 待办
- [x] 写 index.html
- [ ] preview_file + browser audit 390/768/1440
- [ ] acceptance_gate
