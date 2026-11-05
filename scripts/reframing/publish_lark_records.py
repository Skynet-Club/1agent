#!/usr/bin/env python3
"""Publish annotation items into a Feishu/Lark Base table.

Keeps the judgement columns empty: they are what people fill in. Everything else
is content and should not be retyped by hand.

    python3 scripts/reframing/publish_lark_records.py \
        --base-token <token> --table-id <tbl...> \
        --items docs/annotation/reframing-v1/items.jsonl

Requires a configured lark-cli with a user identity that can write to the base.
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

ASK_FIELD = "追问判断"
SUBSTITUTE_FIELD = "替代判断"


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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-token", required=True)
    parser.add_argument("--table-id", required=True)
    parser.add_argument("--items", required=True)
    parser.add_argument("--as", dest="identity", default="user")
    parser.add_argument(
        "--kind",
        choices=["ask", "substitute", "both"],
        default="both",
        help="single-column tables carry only one kind of judgement",
    )
    parser.add_argument(
        "--judge-field",
        default="",
        help="name of the single judgement field, e.g. 你的判断",
    )
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    items = [
        json.loads(line)
        for line in Path(args.items).read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]

    if args.kind != "both":
        suffix = "::ask" if args.kind == "ask" else "::substitute"
        items = [item for item in items if item["case_id"].endswith(suffix)]

    records = []
    for index, item in enumerate(items, start=1):
        is_ask = item["case_id"].endswith("::ask")
        record = {
            "编号": index,
            "原请求": item["stated_request"],
            "助手做了什么": describe(item["model"]),
            # The question must match the options exactly: "should it ask" for
            # the ask rows, "is it acceptable" for the substitute rows. A looser
            # wording lets each person answer a different question.
            "要你判断": (
                "这个请求应该被追问吗？"
                if is_ask
                else "这些替代方案可以接受吗？"
            ),
            "备注": "",
            "case_id": item["case_id"],
        }
        if args.judge_field:
            record[args.judge_field] = ""
        records.append(record)

    payload = {"create_records": records}
    command = [
        "lark-cli",
        "base",
        "+record-batch-create",
        "--as",
        args.identity,
        "--base-token",
        args.base_token,
        "--table-id",
        args.table_id,
        "--json",
        json.dumps(payload, ensure_ascii=False),
    ]
    if args.dry_run:
        command.append("--dry-run")

    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        print(result.stderr.strip() or result.stdout.strip(), file=sys.stderr)
        return 1

    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        print(result.stdout)
        return 0

    created = data.get("data", {}).get("created") or data.get("data", {}).get("records") or []
    print(f"published {len(records)} records, response reports {len(created)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
