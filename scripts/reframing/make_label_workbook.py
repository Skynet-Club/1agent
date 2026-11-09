#!/usr/bin/env python3
"""Build the labelling workbook as an XLSX with real dropdowns.

People should only have to pick from a list. Everything else in the sheet is
read-only: the request, what the assistant did, and the question are content, not
input. The judgement column is the only editable one, apart from a free-text note.

    ~/.cache/xlsxenv/bin/python scripts/reframing/make_label_workbook.py \
        --items docs/annotation/reframing-v1/items.jsonl \
        --out ~/下载/1A-标注材料

Requires openpyxl. The system interpreter is PEP 668 managed, so a dedicated
virtual environment is used rather than installing into it.
"""

import argparse
import json
from pathlib import Path

from openpyxl import Workbook
from openpyxl.styles import Alignment, Border, Font, PatternFill, Protection, Side
from openpyxl.utils import get_column_letter
from openpyxl.worksheet.datavalidation import DataValidation

ASK_OPTIONS = ["应该追问", "不该追问", "说不清"]
SUBSTITUTE_OPTIONS = ["会接受", "不会接受", "说不清"]

HEADERS = ["编号", "原请求", "助手做了什么", "要你判断", "你的判断", "备注", "case_id"]
WIDTHS = [6, 34, 48, 30, 14, 20, 22]
JUDGE_COL = 5
NOTE_COL = 6
CASE_COL = 7


def describe(model):
    kind = "手段" if model.get("classification") == "means" else "目的"
    parts = [f"判断为{kind}"]
    if model.get("question"):
        options = " / ".join(model.get("options") or [])
        parts.append(f"追问「{model['question']}」" + (f"，选项：{options}" if options else ""))
    else:
        parts.append("没有追问")
    if model.get("substitutes"):
        parts.append("替代：" + " / ".join(model["substitutes"]))
    return "；".join(parts)


def build(workbook, items):
    sheet = workbook.active
    sheet.title = "标注"

    thin = Side(style="thin", color="D0D0D0")
    border = Border(left=thin, right=thin, top=thin, bottom=thin)
    header_fill = PatternFill("solid", fgColor="EEF2F7")

    sheet.merge_cells(start_row=1, start_column=1, end_row=1, end_column=CASE_COL)
    sheet.cell(row=1, column=1, value="1A 标注表 · 每行只需在「你的判断」列选一个").font = Font(
        bold=True, size=14
    )
    sheet.merge_cells(start_row=2, start_column=1, end_row=2, end_column=CASE_COL)
    sheet.cell(
        row=2,
        column=1,
        value="全程约 15 分钟。案例全部虚构，不含真实客户信息。两位填完之前不要讨论。",
    ).font = Font(size=10, color="555555")
    sheet.merge_cells(start_row=3, start_column=1, end_row=3, end_column=CASE_COL)
    sheet.cell(
        row=3,
        column=1,
        value="下拉里没有正确答案，选你真实的判断即可；拿不准就选「说不清」，那也是有用的信息。",
    ).font = Font(size=10, color="555555")

    for column, (header, width) in enumerate(zip(HEADERS, WIDTHS), start=1):
        cell = sheet.cell(row=4, column=column, value=header)
        cell.font = Font(bold=True)
        cell.fill = header_fill
        cell.border = border
        cell.alignment = Alignment(horizontal="center", vertical="center")
        sheet.column_dimensions[get_column_letter(column)].width = width

    # A worked example, greyed out, excluded from import because case_id is blank.
    example = ["示例", "我要买个电钻", "判断为手段；追问「打孔是为了穿线还是挂东西」；替代：无线路由器",
               "助手应该追问吗？", "应该追问", "这一行只是示例，不用管", ""]
    for column, value in enumerate(example, start=1):
        cell = sheet.cell(row=5, column=column, value=value)
        cell.font = Font(italic=True, color="888888")
        cell.border = border
        cell.alignment = Alignment(vertical="top", wrap_text=True)

    ask_cells, substitute_cells = [], []
    row = 6
    for index, item in enumerate(items, start=1):
        values = [
            index,
            item["stated_request"],
            describe(item["model"]),
            item["question_for_labeler"],
            None,
            None,
            item["case_id"],
        ]
        for column, value in enumerate(values, start=1):
            cell = sheet.cell(row=row, column=column, value=value)
            cell.border = border
            cell.alignment = Alignment(vertical="top", wrap_text=column in (2, 3, 4, 6))
        sheet.row_dimensions[row].height = 46
        target = ask_cells if item["case_id"].endswith("::ask") else substitute_cells
        target.append(row)
        row += 1

    sheet.column_dimensions[get_column_letter(CASE_COL)].hidden = True
    sheet.freeze_panes = "A5"

    def add_validation(rows, options, title):
        if not rows:
            return
        validation = DataValidation(
            type="list",
            formula1='"' + ",".join(options) + '"',
            allow_blank=True,
            showDropDown=False,  # openpyxl: False actually shows the arrow
            showErrorMessage=True,
            errorTitle="只能从下拉里选",
            error="请点击单元格右侧的小箭头，选择一个。",
            promptTitle=title,
            prompt="点右侧箭头选择，不要手动输入。",
            showInputMessage=True,
        )
        sheet.add_data_validation(validation)
        for row_number in rows:
            validation.add(sheet.cell(row=row_number, column=JUDGE_COL).coordinate)

    add_validation(ask_cells, ASK_OPTIONS, "怎么判断")
    add_validation(substitute_cells, SUBSTITUTE_OPTIONS, "怎么判断")

    # Lock everything except the judgement and note columns of the data rows.
    for r in range(5, row):
        for c in range(1, CASE_COL + 1):
            cell = sheet.cell(row=r, column=c)
            cell.protection = Protection(
                locked=c not in (JUDGE_COL, NOTE_COL) or r == 5
            )
    sheet.protection.sheet = True
    sheet.protection.enable()
    sheet.protection.formatColumns = False
    sheet.protection.formatRows = False


def instructions_sheet(workbook, text):
    sheet = workbook.create_sheet("说明")
    sheet.column_dimensions["A"].width = 100
    for index, line in enumerate(text.splitlines(), start=1):
        cell = sheet.cell(row=index, column=1, value=line)
        cell.alignment = Alignment(wrap_text=True, vertical="top")


MESSAGE = """帮我看 {count} 条判断（约 15 分钟）

背景
我们在做一个会"先问一句再动手"的 AI 助手。
你说"我要买个电钻"，它会问："打孔是为了穿线，还是挂东西？"
如果是穿线联网，它就建议你买路由器，而不是电钻。

这个判断对不对，我们说了不算，所以请两个和项目无关的人各看一眼。

怎么做
1. 打开 01-标注表-A.xlsx（两个人各拿一份，A 和 B）
2. 每一行已经写好了：原请求、助手做了什么、要你判断什么
3. 只需要在「你的判断」那一列点一下，从下拉里选一个
4. 「备注」可以留空，想说明就写一句
5. 填完把整个文件发回给我

规则
- 两个人填完之前不要讨论。讨论后再改，这次测量就作废。
- 下拉里没有"正确答案"，选你真实的判断。
- 拿不准就选「说不清」，那本身是有用的信息。
- 案例全是虚构的，不含任何真实客户数据。

时间不够只填前 {half} 条也行，跟我说一声。谢谢！
"""


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

    guide = (out / "00-说明.md")
    guide_text = guide.read_text(encoding="utf-8") if guide.is_file() else ""

    for sheet_name in ("A", "B"):
        workbook = Workbook()
        build(workbook, items)
        instructions_sheet(workbook, guide_text)
        workbook.save(out / f"01-标注表-{sheet_name}.xlsx")

    (out / "00-说明.txt").write_text(
        MESSAGE.format(count=len(items), half=len(items) // 2), encoding="utf-8"
    )
    print(f"{len(items)} rows -> {out} (xlsx with dropdowns)")


if __name__ == "__main__":
    main()
