#!/usr/bin/env python3
"""Rewrite the 「要你判断」 column in place, keeping existing judgements.

Rebuilding a table would discard what people already filled, so this updates the
question text record by record instead.

    python3 scripts/reframing/update_lark_questions.py \
        --base-token <token> --table-id <tbl...> \
        --items docs/annotation/reframing-v1/items.jsonl
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

QUESTION_FIELD = "要你判断"
CASE_FIELD = "case_id"


def run(arguments):
    result = subprocess.run(arguments, capture_output=True, text=True)
    if result.returncode != 0:
        raise SystemExit(result.stderr.strip() or result.stdout.strip())
    return json.loads(result.stdout)


def question_for(case_id):
    return "这个请求应该被追问吗？" if case_id.endswith("::ask") else "这些替代方案可以接受吗？"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-token", required=True)
    parser.add_argument("--table-id", required=True)
    parser.add_argument("--items", required=True)
    parser.add_argument("--as", dest="identity", default="user")
    args = parser.parse_args()

    known = {
        json.loads(line)["case_id"]
        for line in Path(args.items).read_text(encoding="utf-8").splitlines()
        if line.strip()
    }

    fields = run(
        [
            "lark-cli", "base", "+field-list", "--as", args.identity,
            "--base-token", args.base_token, "--table-id", args.table_id, "--json",
        ]
    )
    id_to_name = {field["id"]: field["name"] for field in fields["data"]["fields"]}

    updates = {}
    offset = 0
    page_size = 200
    while True:
        page = run(
            [
                "lark-cli", "base", "+record-list", "--as", args.identity,
                "--base-token", args.base_token, "--table-id", args.table_id,
                "--page-size", str(page_size), "--offset", str(offset), "--json",
            ]
        )
        rows = page["data"].get("data") or []
        ids = page["data"].get("record_id_list") or []
        order = [id_to_name.get(fid, fid) for fid in page["data"].get("field_id_list") or []]
        for record_id, row in zip(ids, rows):
            record = dict(zip(order, row))
            case_id = str(record.get(CASE_FIELD) or "").strip()
            if case_id in known:
                updates[record_id] = {QUESTION_FIELD: question_for(case_id)}
        if len(rows) < page_size:
            break
        offset += page_size

    for start in range(0, len(updates), 100):
        chunk = dict(list(updates.items())[start : start + 100])
        run(
            [
                "lark-cli", "base", "+record-batch-update", "--as", args.identity,
                "--base-token", args.base_token, "--table-id", args.table_id,
                "--json", json.dumps({"update_records": chunk}, ensure_ascii=False),
            ]
        )
    print(f"updated {len(updates)} records in {args.table_id}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
