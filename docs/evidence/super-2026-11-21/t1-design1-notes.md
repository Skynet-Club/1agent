# NOTES — 宋庄晨启教育 · 加入我们（移动端网页）

## 目标
把飞书 wiki《加入我们微页面文案》做成一个手机端可浏览、可交互的单页网页，交付 `/tmp/1agent-t1-design1/index.html`，并按 unslop 做质检。

## 已知事实
- 文档：https://szchenqi.feishu.cn/wiki/QDPswzhW7irkiSkEq8McFXP1nMc ，document_id Higbd259domECKx3XuZcinYWnEg，revision 990。
- 本会话 terminal_create 连续 4 次失败：`session_limit：活动会话最多 128 个` → 无法在本会话内跑 `lark-cli`，也无法 `cp` 二进制资源。
- 内容来源：早前会话用 lark-cli 导出的同一文档 JSON 快照 `/home/skynet/work/join-us/source.md`（sha256 d18866b6d8c53358c7b468eb23242d6b1f5ad4577a43c9f614f433749d32f387，revision 990）。
- 图片是 feishu file token（需鉴权），fetch_url 拿不到；write_file 只能写文本 → 页面必须不依赖文档原图。
- 不得修改 /home/skynet/work/join-us（只读引用）。

## 决策
1. 视觉不复用原图，用 SVG/CSS 画"花海地平线与砖墙"这一幕，避免依赖不可获得的素材。
2. 字体走系统 CJK 栈（衬线标题 + 无衬线正文），不外链 CDN，保证离线可渲染。
3. 交互：岗位切换 tabs、职责/要求 accordion、复制电话/邮箱、底部常驻 CTA、地点复制。
4. 微信二维码不可得 → 改为"二维码见原招聘文档"并给原文链接，不编造微信号。

## 待办
- [ ] index.html
- [ ] preview_file 200 + sha
- [ ] browser audit 390/768/1440（scrollWidth<=clientWidth、tap targets、placeholder）
- [ ] unslop 质检清单
- [ ] acceptance_gate

## 未决
- 文档 revision 是否已从 990 前进（无 shell，无法用 lark-cli 复查最新 revision）。
