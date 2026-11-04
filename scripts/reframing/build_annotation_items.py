#!/usr/bin/env python3
"""Build blind annotation items from one pilot run.

Two judgements per scored case, encoded as two case ids so the existing
`1agent annotate` store and Cohen's kappa can be reused unchanged:

    <case>::ask         should the assistant have asked a question here?
    <case>::substitute  is at least one proposed substitute acceptable?

The expected classification is deliberately omitted so labelers are blind to
our own ground truth.

Usage:
    python3 scripts/reframing/build_annotation_items.py \
        --responses ~/.local/state/1agent-reframing/<stamp>/responses.jsonl \
        --fixture fixtures/reframing/heldout-v2.json \
        --out docs/annotation/reframing-v1
"""

import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--responses", required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--rep", type=int, default=1)
    args = parser.parse_args()

    fixture = json.loads(Path(args.fixture).read_text(encoding="utf-8"))
    cases = {case["id"]: case for case in fixture["cases"]}

    chosen = {}
    for line in Path(args.responses).read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        record = json.loads(line)
        if record.get("rep") != args.rep or not record.get("evaluation"):
            continue
        chosen[record["case"]] = record

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    items = []
    for case_id, record in chosen.items():
        case = cases[case_id]
        items.append(
            {
                "case_id": f"{case_id}::ask",
                "stated_request": case["stated_request"],
                "constraints": case.get("constraints") or [],
                "model": {
                    "classification": record.get("classification"),
                    "question": record.get("question"),
                    "options": record.get("options") or [],
                    "substitutes": record.get("substitutes") or [],
                },
                "question_for_labeler": (
                    "助手在这个请求上追问了一个问题。应该追问吗？"
                    if record.get("question")
                    else "助手在这个请求上没有追问，直接按原请求处理。应该追问吗？"
                ),
            }
        )
        # An end has no substitute by construction, so asking whether "no
        # substitute is acceptable" is a trivially true question. Only means
        # proposals get the substitute judgement.
        substitutes = record.get("substitutes") or []
        if record.get("classification") != "means":
            continue
        items.append(
            {
                "case_id": f"{case_id}::substitute",
                "stated_request": case["stated_request"],
                "constraints": case.get("constraints") or [],
                "model": {
                    "classification": record.get("classification"),
                    "question": record.get("question"),
                    "options": record.get("options") or [],
                    "substitutes": substitutes,
                },
                "question_for_labeler": (
                    "至少有一个替代方案是用户会接受的吗？"
                    if substitutes
                    else "助手没有给出替代方案。这是否可接受？"
                ),
            }
        )

    path = out / "items.jsonl"
    with path.open("w", encoding="utf-8") as handle:
        for item in items:
            handle.write(json.dumps(item, ensure_ascii=False) + "\n")
    print(f"{len(items)} items -> {path}")

    template = out / "labels-template.csv"
    with template.open("w", encoding="utf-8") as handle:
        handle.write("case_id,label,note\n")
        for item in items:
            handle.write(f"{item['case_id']},,\n")
    print(f"label template -> {template}")


if __name__ == "__main__":
    main()
