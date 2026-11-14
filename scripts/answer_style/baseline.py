#!/usr/bin/env python3
"""Read-only baseline for answer style.

Scans journal files and reports, per distinct answer conclusion:

  chars, sentences, longest verbatim overlap with the request,
  scaffolding hits, hedge hits, meta hits, enumerated lines, summary ending.

These are PROXIES. The contract's real measure is "would deleting this line
change the user's next action", which needs judgement, not a regex. This script
measures the markers that correlate with it and must not be reported as the
contract metric itself.

Nothing is written and no network is used.

Usage:
    python3 scripts/answer_style/baseline.py [--journal-dir .journal] [--json]
"""

import argparse
import difflib
import hashlib
import json
import re
import statistics
import sys
from pathlib import Path

SCAFFOLDING = [
    "当然",
    "首先",
    "其次",
    "总的来说",
    "总之",
    "综上所述",
    "希望",
    "以下是",
    "让我们",
    "需要注意的是",
    "值得一提的是",
    "简单来说",
    "换句话说",
]
HEDGES = ["可能因人而异", "因人而异", "具体问题具体分析", "视情况而定", "不一定"]
META = ["我理解", "您希望", "你的需求是", "根据您的要求", "如您所述"]
SUMMARY_ENDINGS = ["综上", "总之", "希望", "以上", "总结"]
SENTENCE_SPLIT = re.compile(r"[。！？!?；;\n]+")
ENUMERATED = re.compile(r"^\s*(?:\d+[.、)]|[A-Ca-c][.、)]|[-*])\s+")
FALLBACK_MARKER = "不对自然语言任务作可靠性判断"


def normalize(text):
    return re.sub(r"[\s\W]+", "", text, flags=re.UNICODE)


def load_answers(journal_dir):
    rows = []
    for path in sorted(Path(journal_dir).glob("*.jsonl")):
        objective = None
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if not line.strip():
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError:
                continue
            event = record.get("event") or {}
            kind = event.get("type")
            data = event.get("data") or {}
            if kind == "run_started":
                objective = (data.get("request") or {}).get("objective")
            elif kind == "answer_produced":
                answer = data.get("answer") or {}
                conclusion = answer.get("conclusion") or ""
                rows.append(
                    {
                        "journal": path.name,
                        "objective": objective or "",
                        "conclusion": conclusion,
                        "status": answer.get("status"),
                    }
                )
    return rows


def classify(row):
    text = row["conclusion"]
    if FALLBACK_MARKER in text:
        return "fallback"
    if text.startswith("证据读取失败"):
        return "evidence_error"
    if len(normalize(text)) < 20:
        return "trivial"
    return "real"


FENCED = re.compile(r"```.*?```", re.S)


def measure(row):
    text = row["conclusion"]
    # Code blocks are not prose: counting their lines as sentences inflates every
    # length metric and hides how much of the answer is actually writing.
    prose = FENCED.sub(" ", text)
    sentences = [s.strip() for s in SENTENCE_SPLIT.split(prose) if s.strip()]
    objective = normalize(row["objective"])
    conclusion = normalize(prose)
    matcher = difflib.SequenceMatcher(None, objective, conclusion, autojunk=False)
    longest = matcher.find_longest_match(0, len(objective), 0, len(conclusion)).size
    # Compare CJK characters against Latin *words*, not letters: an English word
    # is several letters long, so a letter ratio marks Chinese answers that
    # mention code identifiers as English.
    cjk = len(re.findall(r"[\u4e00-\u9fff]", prose))
    latin_words = len(re.findall(r"[A-Za-z][A-Za-z'-]*", prose))
    total = cjk + latin_words * 2
    answer_share = round(cjk / total, 3) if total else 0.0

    # A language mismatch is relative to the request, not to an assumption that
    # the user writes Chinese. Ratios are unreliable here: a Chinese request that
    # names Python, C, Java and Hello can score below half CJK. Use presence
    # instead. A Chinese request needs a substantially Chinese answer; an English
    # request needs one that is not mostly Chinese.
    request_has_cjk = re.search(r"[\u4e00-\u9fff]", row["objective"]) is not None
    if request_has_cjk:
        mismatch = cjk < max(5, int(total * 0.15))
    else:
        mismatch = cjk > total * 0.3
    return {
        "cjk_share": answer_share,
        "language_mismatch": mismatch,
        "chars": len(text),
        "prose_chars": len(prose),
        "sentences": len(sentences),
        "longest_restatement": longest,
        "scaffolding": sum(text.count(word) for word in SCAFFOLDING),
        "hedges": sum(text.count(word) for word in HEDGES),
        "meta": sum(text.count(word) for word in META),
        "enumerated": sum(1 for line in text.splitlines() if ENUMERATED.match(line)),
        "summary_ending": any(
            sentences and sentences[-1].startswith(word) for word in SUMMARY_ENDINGS
        ),
        "kind": classify(row),
    }


def summarize(rows, key):
    values = [row[key] for row in rows]
    if not values:
        return {"n": 0}
    ordered = sorted(values)
    return {
        "n": len(values),
        "median": statistics.median(values),
        "p90": ordered[max(0, int(len(ordered) * 0.9) - 1)],
        "max": ordered[-1],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--journal-dir", default=".journal")
    parser.add_argument("--json", action="store_true")
    parser.add_argument(
        "--list-objectives",
        action="store_true",
        help="print the distinct real objectives, one JSON string per line",
    )
    args = parser.parse_args()

    rows = load_answers(args.journal_dir)
    if not rows:
        print(f"no answer events under {args.journal_dir}", file=sys.stderr)
        return 1

    for row in rows:
        row.update(measure(row))

    kinds = {}
    for row in rows:
        kinds[row["kind"]] = kinds.get(row["kind"], 0) + 1

    distinct = {}
    for row in rows:
        digest = hashlib.sha256(row["conclusion"].encode("utf-8")).hexdigest()[:12]
        distinct.setdefault(digest, row)
    real = [row for row in distinct.values() if row["kind"] == "real"]

    report = {
        "journal_dir": str(Path(args.journal_dir).resolve()),
        "answer_events": len(rows),
        "distinct_conclusions": len(distinct),
        "by_kind": kinds,
        "measurable_real": len(real),
        "real_metrics": {
            key: summarize(real, key)
            for key in [
                "chars",
                "prose_chars",
                "sentences",
                "longest_restatement",
                "scaffolding",
                "hedges",
                "meta",
                "enumerated",
            ]
        },
        "real_with_scaffolding": sum(1 for row in real if row["scaffolding"] > 0),
        "real_with_restatement_over_8": sum(
            1 for row in real if row["longest_restatement"] > 8
        ),
        "real_with_summary_ending": sum(1 for row in real if row["summary_ending"]),
        "real_with_menu": sum(1 for row in real if row["enumerated"] >= 2),
        "real_language_mismatch": sum(1 for row in real if row["language_mismatch"]),
    }

    if args.list_objectives:
        seen = set()
        for row in distinct.values():
            if row["kind"] != "real":
                continue
            objective = row["objective"].strip()
            if objective and objective not in seen:
                seen.add(objective)
                print(json.dumps(objective, ensure_ascii=False))
        return 0

    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0

    print(f"journal dir        {report['journal_dir']}")
    print(f"answer events      {report['answer_events']}")
    print(f"distinct answers   {report['distinct_conclusions']}")
    for kind, count in sorted(kinds.items(), key=lambda item: -item[1]):
        print(f"  {kind:<14} {count}")
    print(f"measurable real    {report['measurable_real']}")
    if real:
        for key in ["chars", "prose_chars", "sentences", "longest_restatement", "enumerated"]:
            stats = report["real_metrics"][key]
            print(
                f"  {key:<20} median {stats['median']}  p90 {stats['p90']}  max {stats['max']}"
            )
        print(f"  with scaffolding     {report['real_with_scaffolding']}")
        print(f"  restatement > 8      {report['real_with_restatement_over_8']}")
        print(f"  summary ending       {report['real_with_summary_ending']}")
    else:
        print("  no measurable real answers: the corpus cannot support a baseline")
    return 0


if __name__ == "__main__":
    sys.exit(main())
