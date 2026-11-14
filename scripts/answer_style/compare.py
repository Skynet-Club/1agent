#!/usr/bin/env python3
"""Paired comparison of answer style between two journal directories.

Matches answers by their objective, so the comparison is on the same tasks
rather than on two differently-shaped samples.

Usage:
    python3 scripts/answer_style/compare.py .journal .journal-style-post
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from baseline import load_answers, measure  # noqa: E402


def real_by_objective(directory):
    rows = [row for row in load_answers(directory) if row["objective"].strip()]
    for row in rows:
        row.update(measure(row))
    selected = {}
    for row in rows:
        if row["kind"] != "real":
            continue
        selected.setdefault(row["objective"].strip(), row)
    return selected


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("before")
    parser.add_argument("after")
    args = parser.parse_args()

    before = real_by_objective(args.before)
    after = real_by_objective(args.after)
    shared = sorted(set(before) & set(after))

    if not shared:
        print("no shared objectives: nothing can be compared", file=sys.stderr)
        return 1

    print(f"before {len(before)} objectives, after {len(after)}, paired {len(shared)}")
    print()
    header = f"{'#':<3}{'chars before':>13}{'chars after':>12}{'delta':>8}{'sent. b':>9}{'sent. a':>8}{'restate b':>11}{'restate a':>11}"
    print(header)
    print("-" * len(header))

    deltas = []
    sentence_deltas = []
    restatement_before = 0
    restatement_after = 0
    for index, objective in enumerate(shared, start=1):
        old = before[objective]
        new = after[objective]
        delta = new["prose_chars"] - old["prose_chars"]
        deltas.append(delta)
        sentence_deltas.append(new["sentences"] - old["sentences"])
        restatement_before += 1 if old["longest_restatement"] > 8 else 0
        restatement_after += 1 if new["longest_restatement"] > 8 else 0
        print(
            f"{index:<3}{old['prose_chars']:>13}{new['prose_chars']:>12}{delta:>+8}"
            f"{old['sentences']:>9}{new['sentences']:>8}"
            f"{old['longest_restatement']:>11}{new['longest_restatement']:>11}"
        )

    deltas.sort()
    median = deltas[len(deltas) // 2] if len(deltas) % 2 else (
        deltas[len(deltas) // 2 - 1] + deltas[len(deltas) // 2]
    ) / 2

    print()
    print(f"median prose-char delta : {median:+.0f}")
    print(f"shorter after           : {sum(1 for d in deltas if d < 0)}/{len(deltas)}")
    print(f"restatement > 8         : {restatement_before} -> {restatement_after}")
    latin_before = sum(1 for o in shared if before[o]["cjk_share"] < 0.5)
    latin_after = sum(1 for o in shared if after[o]["cjk_share"] < 0.5)
    print(f"latin-dominant answers  : {latin_before} -> {latin_after}")
    print(
        f"median sentence delta   : {sorted(sentence_deltas)[len(sentence_deltas) // 2]:+d}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
