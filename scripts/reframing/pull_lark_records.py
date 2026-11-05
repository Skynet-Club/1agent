#!/usr/bin/env python3
"""Pull filled judgements from a Feishu/Lark Base table into the CSV the
importer already understands.

    python3 scripts/reframing/pull_lark_records.py \
        --base-token <token> --table-id <tbl...> --out ~/下载/labels-A.csv

Blank rows are kept so the import step reports how many are still unfilled.
The select options are plain Chinese; the mapping to the three internal labels
lives here and nowhere else.
"""

import argparse
import csv
import json
import subprocess
import sys

ASK_FIELD = "追问判断"
SUBSTITUTE_FIELD = "替代判断"
NOTE_FIELD = "备注"
CASE_FIELD = "case_id"

ASK_MAP = {"应该追问": "supported", "不该追问": "unsupported", "说不清": "needs_evidence"}
SUBSTITUTE_MAP = {
    "可接受": "supported",
    "不可接受": "unsupported",
    "说不清": "needs_evidence",
}


def run(arguments):
    result = subprocess.run(arguments, capture_output=True, text=True)
    if result.returncode != 0:
        raise SystemExit(result.stderr.strip() or result.stdout.strip())
    return json.loads(result.stdout)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-token", required=True)
    parser.add_argument("--table-id", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--as", dest="identity", default="user")
    parser.add_argument("--page-size", type=int, default=200)
    args = parser.parse_args()

    fields = run(
        [
            "lark-cli",
            "base",
            "+field-list",
            "--as",
            args.identity,
            "--base-token",
            args.base_token,
            "--table-id",
            args.table_id,
            "--json",
        ]
    )
    # record rows are positional, and their order is the field_id_list returned
    # with each page, not the order of this listing.
    id_to_name = {field["id"]: field["name"] for field in fields["data"]["fields"]}

    records = []
    offset = 0
    while True:
        page = run(
            [
                "lark-cli",
                "base",
                "+record-list",
                "--as",
                args.identity,
                "--base-token",
                args.base_token,
                "--table-id",
                args.table_id,
                "--page-size",
                str(args.page_size),
                "--offset",
                str(offset),
                "--json",
            ]
        )
        rows = page["data"].get("data") or []
        order = page["data"].get("field_id_list") or []
        names = [id_to_name.get(field_id, field_id) for field_id in order]
        for row in rows:
            records.append(dict(zip(names, row)))
        if len(rows) < args.page_size:
            break
        offset += args.page_size

    filled = 0
    with open(args.out, "w", encoding="utf-8-sig", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["case_id", "label", "note"])
        for record in records:
            case_id = (record.get(CASE_FIELD) or "").strip()
            if not case_id:
                continue
            # Pick the field by row type. Falling back to "whichever is filled"
            # silently reads the wrong column when a person fills both, which is
            # exactly what happened in the first round.
            if "你的判断" in record:
                # Single-column tables: the row type already says which question
                # applies, so the same field name cannot be misread.
                raw = record.get("你的判断") or ""
            else:
                field = ASK_FIELD if case_id.endswith("::ask") else SUBSTITUTE_FIELD
                raw = record.get(field) or ""
            if isinstance(raw, list):
                raw = raw[0] if raw else ""
            option = str(raw).strip()
            label = ASK_MAP.get(option) or SUBSTITUTE_MAP.get(option) or ""
            if label:
                filled += 1
            writer.writerow([case_id, label, record.get(NOTE_FIELD) or ""])

    print(f"records {len(records)}, filled {filled} -> {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
