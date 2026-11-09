#!/usr/bin/env python3
"""Build self-contained labelling sheets from annotation items.

A sheet that only contains case ids is an index, not a form. Each row here
carries the request, what the assistant actually did, the question to answer,
and a blank to fill. No cross-referencing.

    python3 scripts/reframing/make_label_sheets.py \
        --items docs/annotation/reframing-v1/items.jsonl \
        --out ~/下载/1A-标注材料
"""

import argparse
import csv
import json
from pathlib import Path

BOM = "\ufeff"  # so spreadsheet software opens UTF-8 instead of guessing GBK
LABELS = "supported / unsupported / needs_evidence"


def describe_model(model):
    classification = "手段" if model.get("classification") == "means" else "目的"
    parts = [f"判断为{classification}"]
    if model.get("question"):
        options = " / ".join(model.get("options") or [])
        parts.append(f"追问「{model['question']}」" + (f"，选项：{options}" if options else ""))
    else:
        parts.append("没有追问")
    if model.get("substitutes"):
        parts.append("替代：" + " / ".join(model["substitutes"]))
    return "；".join(parts)


def rows(items):
    for index, item in enumerate(items, start=1):
        yield {
            "编号": str(index),
            "要填的列": "",
            "原请求": item["stated_request"],
            "助手做了什么": describe_model(item["model"]),
            "要你判断": item["question_for_labeler"],
            "label": "?",
            "note": "",
            "case_id": item["case_id"],
        }


FIELDS = ["编号", "原请求", "助手做了什么", "要你判断", "label", "note", "case_id"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--items", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    items = [
        json.loads(line)
        for line in Path(args.items).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    out = Path(args.out).expanduser()
    out.mkdir(parents=True, exist_ok=True)

    for sheet in ("A", "B"):
        path = out / f"01-标注表-{sheet}.csv"
        with path.open("w", encoding="utf-8-sig", newline="") as handle:
            writer = csv.DictWriter(handle, fieldnames=FIELDS, quoting=csv.QUOTE_MINIMAL)
            writer.writeheader()
            for row in rows(items):
                writer.writerow({key: row[key] for key in FIELDS})

    lines = [
        "# 标注表",
        "",
        f"共 {len(items)} 条。把每行 `label` 后面的 `?` 换成下面三个词之一：",
        "",
        "```",
        f"{LABELS}",
        "```",
        "",
        "`note` 可以留空；填 `unsupported` 或 `needs_evidence` 时写一句理由。",
        "",
        "| 编号 | 原请求 | 助手做了什么 | 要你判断 | label | note | case_id |",
        "|---|---|---|---|---|---|---|",
    ]
    for row in rows(items):
        lines.append(
            f"| {row['编号']} | {row['原请求']} | {row['助手做了什么']} | "
            f"{row['要你判断']} | ? | | `{row['case_id']}` |"
        )
    (out / "01-标注表.md").write_text("\n".join(lines) + "\n", encoding="utf-8")

    # The message text is meant to be pasted into chat or mail as-is. The count is
    # injected so it cannot drift from the sheet.
    message = f"""【请帮我看 {len(items)} 条判断，大约 15 分钟】

背景
我们在做一个会"先问一句再动手"的 AI 助手。
你说"我要买个电钻"，它会问："打孔是为了穿线，还是挂东西？"
如果是穿线联网，它就建议你买路由器，而不是电钻。

这个判断对不对，我们说了不算。
所以想请两个和项目无关的人，各自独立看一眼。

要做的（只有一步）
1. 打开 01-标注表-A.csv，两人各拿一份（A 和 B）
2. 每一行已经写好了：原请求、助手做了什么、要你判断什么
3. 把 label 那一列的 ? 换成下面三个词之一：

   supported       判断为"对"（该问 / 该沉默 / 替代可接受）
   unsupported     判断为"错"
   needs_evidence  说不清，两种都成立

4. note 可以留空；填后两个时最好写一句理由
5. 填完把整份表格发回给我，文件名不用改

三条规则
- 两个人填完之前不要讨论。讨论后再改，这次测量就作废了。
- 别填"挺好的"。选不出来就写 needs_evidence，那本身是有用的信息。
- 案例全是虚构的，不含任何真实客户数据，也不需要懂技术。

小提示
表格是 UTF-8 编码。如果打开是乱码，用 WPS / 记事本打开时
手动选 UTF-8 就行。不想用表格的话，同目录的 01-标注表.md
内容一样，直接在文本编辑器里改 ? 也可以。

时间不够的话，只填前 {len(items) // 2} 条也行，跟我说一声。

谢谢！
"""
    (out / "00-说明.txt").write_text(message, encoding="utf-8")
    print(f"{len(items)} rows -> {out}")


if __name__ == "__main__":
    main()
