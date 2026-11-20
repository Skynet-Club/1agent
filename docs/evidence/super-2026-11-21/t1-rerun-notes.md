# 加入我们微页面 · rerun10 工作笔记

## 目标
把飞书文档《加入我们微页面文案》（https://szchenqi.feishu.cn/wiki/QDPswzhW7irkiSkEq8McFXP1nMc ，revision 990）做成手机优先、可交互的单文件网页，交付 `/tmp/1agent-t1-rerun10/index.html`，并过 unslop 质检。

## 硬约束
- 只写 `/tmp/1agent-t1-rerun10/index.html`；不改 `/home/skynet/work/join-us`。
- 手机 390 优先；390/768/1440 三档 `scrollWidth == clientWidth`；触控目标 ≥44px；无占位符。
- 文案以文档原文为准，不加文档没有的事实。

## 工具链现状（失败记录）
1. `terminal_create` 连续两次失败：`session_limit：活动会话最多 128 个` → 本 run 无法执行任何 shell，因此 **无法现场跑 lark-cli**。
2. `spawn_agent` 失败：`no provider route for mode Execute` → 无法用子 agent 代跑 shell。
3. `skill_list` / `skill_search` 返回 0：注册表里没有 `ui-ux-pro-max`、`unslop`；`ui-ux-pro-max` 在 `~/.hermes/skills` 下不存在（只有 catalog 条目）。
4. 替代证据：`/home/skynet/work/join-us/source.md` 就是同一 wiki 节点（`QDPswzhW7irkiSkEq8McFXP1nMc`，revision 990）此前用 lark-cli 抓下来的 JSON 原文，含正文与评论。本次以它作为文档内容来源，未声称现场跑过 lark-cli。
5. 实际可用的质检能力：`~/.hermes/skills/creative/unslop/SKILL.md`（v1.0.0，CC BY 4.0）+ `browser audit`（真实 390/768/1440 视口测量）。

## 设计决策
- 方向五句：产品=宋庄晨启教育招教学岗与运营岗；性格=温厚、具体、有手作感；世界=艺术小镇（红砖店、白桌子、颜料、五分钟外的花田）；时刻=首屏地平线插画 + 三段第一人称引文；记号=编号色卡（同色相四阶）。
- 因为本 run 拿不到二进制资产（无 shell，不能 base64 内联 `img/`），页面不引用任何图片：只用内联 SVG 地平线 + 版式承载；文档里的实拍与二维码不放进本页，联系方式用电话/邮箱原文。
- 一个强调色（砖红 #9C3B2E），暖纸底，衬线标题 + 系统无衬线正文；无渐变、无发光、无 emoji、无阴影。
- 交互：章节 scroll-spy、晨启人说横向 snap 轮播 + 圆点、岗位 tab（方向键可切）、邮箱复制 + toast、底部常驻操作条、`prefers-reduced-motion` 全关动效。

## 待办
- [ ] 写 index.html
- [ ] preview_file + browser open/audit（390/768/1440）
- [ ] audit 不达标则修
- [ ] acceptance_gate
