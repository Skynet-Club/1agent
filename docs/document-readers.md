# 文档阅读能力矩阵

`read_document` 是本地文档的统一只读入口；`skill_search` 是 skill 发现入口。
两者都不写文件、不激活 skill、不执行 skill。具体 skill 由 Worker 应用。

## `read_document`

| 类别 | 格式 | 实现 | 状态 |
|---|---|---|---|
| 纯文本/代码/数据 | txt, md, markdown, rst, log, csv, tsv, json, jsonl, yaml/yml, toml, ini/cfg/conf, tex, sql, sh, py, rs, js/ts/tsx/jsx, go, java, c/cc/cpp/h/hpp, css/scss/less, srt/vtt, eml, ipynb | UTF-8 直接读取 | ✅ |
| 标记语言 | html/htm/xhtml | 去 script/style、标签转文本、实体解码 | ✅ |
| 标记语言 | xml/svg | 原文（保留结构） | ✅ |
| PDF | pdf | `pdftotext -layout`（poppler-utils） | ✅ 文本型 PDF；扫描件需 OCR |
| Word | docx | ZIP + `word/document.xml`、header/footer 提取 | ✅ |
| PowerPoint | pptx | ZIP + `ppt/slides/slide*.xml` 提取 | ✅ |
| Excel | xlsx | ZIP + sharedStrings/worksheets 提取单元格 | ✅ 基础单元格；复杂公式/图表只出值 |
| OpenDocument | odt, ods, odp | ZIP + `content.xml` 提取 | ✅ |
| EPUB | epub | ZIP + XHTML/HTML 提取 | ✅ |
| RTF | rtf | 控制字/转义/段落解析 | ✅ 基础文本 |
| 旧版 Office | doc, xls, ppt | `catdoc` / `xls2csv` / `catppt` | ⚠️ 需安装 catdoc |
| 图片 | png, jpg/jpeg, webp, gif, bmp, tif/tiff | `tesseract` 或 `OCR_PYTHON`（RapidOCR） | ⚠️ 需 OCR 后端 |
| 压缩包 | zip, tar, tgz, gz | `unzip -Z1` / `tar -tf` 列目录 | ✅ 只列条目，不自动解压 |
| SQLite | sqlite, db | `sqlite3 <path> .tables` | ⚠️ 需 sqlite3 |
| 音视频 | mp3, wav, m4a, flac, ogg, mp4, mov, mkv | whisper CLI | ⚠️ 未配置 whisper |
| 其他 UTF-8 文本 | 未列出的扩展名 | 尝试 UTF-8；失败则报不支持 | ✅/⚠️ |

### 安装提示

```bash
# 图片 OCR（任一）
sudo apt install tesseract-ocr tesseract-ocr-chi-sim
# 或隔离 venv + RapidOCR：
uv venv ~/.local/share/1agent-ocr
uv pip install --python ~/.local/share/1agent-ocr/bin/python rapidocr_onnxruntime
export OCR_PYTHON=~/.local/share/1agent-ocr/bin/python

# 旧版 Office
sudo apt install catdoc

# SQLite
sudo apt install sqlite3

# 音视频转写
# 安装 whisper.cpp 或兼容 CLI，并在工具中接入
```

## `skill_search`

搜索来源：

1. 1Agent skill journal（`ONEAGENT_SKILL_JOURNAL`，默认 `~/.1a/skills.jsonl`）；
2. `ONEAGENT_SKILL_ROOTS`（冒号分隔）；
3. 默认根：`~/.claude/skills`、`~/.claude/plugins/marketplaces`、`~/.1a/skills`。

返回：`name`、`description`、`version`、`status`、`source`、`path`、`requires_tools`、
`risk`、`body`（有界）。规划器用它发现 skill，把 skill 说明写入 Work Item objective，
把声明的工具写入 `required_tools`；**不在规划阶段执行 skill**。

`lark-cli` 只是用户安装的一个外部 skill/CLI，不是产品内置能力。产品不硬编码 lark；
如果 `lark-doc` 等 SKILL.md 存在，`skill_search` 会找到；Worker 按 skill 说明调用
`lark-cli`。规划阶段需要读取 Lark 文档时，应由一个显式 Work Item 承担，而不是把
某个 skill 写进规划器代码。

## 边界

- 加密/密码保护的 PDF、Word、Excel 不在支持范围。
- 扫描版 PDF 目前只走 `pdftotext`；没有 OCR 就得到空文本，不会伪造内容。
- 图片 OCR 需要后端；没有后端时明确报错，不返回二进制噪声。
- 旧版 `.doc/.xls/.ppt` 需要 catdoc；没有时明确报安装提示。
- 音视频需要 whisper；当前未配置。
- `read_document` 只能读 `PermissionSet.read_roots` 内的普通文件；拒绝符号链接。
- `skill_search` 只读 skill 根和 journal；不写、不激活、不执行。
- 飞书等远端文档不在 `read_document` 范围；由 `skill_exec` 调用 `lark-cli`
  等操作者声明的只读命令读取，见 `docs/skill-exec.md`。
