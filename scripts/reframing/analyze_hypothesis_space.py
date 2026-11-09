#!/usr/bin/env python3
"""Research tool: does the hypothesis space resolve in <=2 questions?

Reads fixtures/reframing/hypothesis-space-v0.json and computes, per case:

  k            number of candidate hypotheses
  a            number of distinct ACTION classes (action equivalence merges them)
  E[depth]     optimal expected number of questions, priors weighted
  W[depth]     optimal worst-case number of questions (minimax)
  H_bound      entropy lower bound: H(actions) / log2(max_branch_arity)
  naive        strawman baseline: one binary question per extra hypothesis (k-1)

Terminal condition: a state is resolved when every remaining hypothesis leads to
the same ACTION. Resolving an action is what the user needs; resolving the
hypothesis identity is not.

This is not product code and is not wired into the runtime.
"""

import json
import math
import sys
from functools import lru_cache
from pathlib import Path

FIXTURE = Path(__file__).resolve().parents[2] / "fixtures/reframing/hypothesis-space-v0.json"


def fixture_path(argv):
    return Path(argv[1]).resolve() if len(argv) > 1 else FIXTURE


def load(path):
    with path.open(encoding="utf-8") as handle:
        return json.load(handle)


def validate(case):
    errors = []
    hids = [h["id"] for h in case["hypotheses"]]
    if len(set(hids)) != len(hids):
        errors.append(f"{case['id']}: duplicate hypothesis id")
    prior = sum(h["prior"] for h in case["hypotheses"])
    if abs(prior - 1.0) > 1e-6:
        errors.append(f"{case['id']}: priors sum to {prior}, expected 1.0")
    known = set(hids)
    for q in case["questions"]:
        seen = []
        for label, members in q["branches"].items():
            if not members:
                errors.append(f"{case['id']}/{q['id']}: empty branch '{label}'")
            for member in members:
                if member not in known:
                    errors.append(f"{case['id']}/{q['id']}: unknown hypothesis {member}")
                seen.append(member)
        if len(seen) != len(set(seen)):
            errors.append(f"{case['id']}/{q['id']}: hypothesis appears in two branches")
    return errors


class Case:
    def __init__(self, raw):
        self.id = raw["id"]
        self.prior = {h["id"]: h["prior"] for h in raw["hypotheses"]}
        self.action = {h["id"]: h["action"] for h in raw["hypotheses"]}
        self.questions = raw["questions"]
        self.all = frozenset(self.prior)
        self.k = len(self.all)
        self.actions = len({self.action[h] for h in self.all})

    def partition(self, state, question):
        """A question is applicable to a state only if its branches cover it.

        Partial coverage would silently drop the uncovered hypotheses and make the
        depth look smaller than it is, so it is rejected rather than ignored.
        """
        parts = []
        covered = set()
        for label, members in question["branches"].items():
            chunk = state & frozenset(members)
            if chunk:
                parts.append((label, frozenset(chunk)))
                covered |= chunk
        if covered != set(state):
            return []
        return parts

    def resolved(self, state):
        return len({self.action[h] for h in state}) <= 1

    def w(self, state):
        if self.resolved(state):
            return 0, None
        best, choice = math.inf, None
        for question in self.questions:
            parts = self.partition(state, question)
            if len(parts) < 2:
                continue
            cost = 1 + max(self.w(chunk)[0] for _, chunk in parts)
            if cost < best:
                best, choice = cost, question["id"]
        return best, choice

    def e(self, state):
        if self.resolved(state):
            return 0.0, None
        total = sum(self.prior[h] for h in state)
        best, choice = math.inf, None
        for question in self.questions:
            parts = self.partition(state, question)
            if len(parts) < 2:
                continue
            cost = 0.0
            for _, chunk in parts:
                weight = sum(self.prior[h] for h in chunk) / total
                cost += weight * self.e(chunk)[0]
            cost += 1.0
            if cost < best:
                best, choice = cost, question["id"]
        return best, choice

    def entropy_actions(self):
        grouped = {}
        for hid, action in self.action.items():
            grouped[action] = grouped.get(action, 0.0) + self.prior[hid]
        return -sum(p * math.log2(p) for p in grouped.values() if p > 0)


def main():
    fixture = load(fixture_path(sys.argv))
    arity = fixture.get("max_branch_arity", 2)
    errors = []
    for raw in fixture["cases"]:
        errors.extend(validate(raw))
    if errors:
        print("FIXTURE INVALID")
        for error in errors:
            print("  " + error)
        return 1

    rows = []
    for raw in fixture["cases"]:
        case = Case(raw)
        worst, first = case.w(case.all)
        expected, _ = case.e(case.all)
        bound = abs(case.entropy_actions() / math.log2(arity)) if arity > 1 else 0.0
        naive = max(case.k - 1, 0)
        rows.append(
            {
                "id": case.id,
                "k": case.k,
                "a": case.actions,
                "first": first or "-",
                "expected": expected,
                "worst": worst,
                "bound": bound,
                "naive": naive,
            }
        )

    header = f"{'case':<24}{'k':>3}{'a':>3}{'E[depth]':>10}{'W[depth]':>10}{'H-bound':>10}{'naive':>7}  first question"
    print(header)
    print("-" * len(header))
    for row in rows:
        print(
            f"{row['id']:<24}{row['k']:>3}{row['a']:>3}"
            f"{row['expected']:>10.2f}{row['worst']:>10}{row['bound']:>10.2f}{row['naive']:>7}"
            f"  {row['first']}"
        )

    n = len(rows)
    mean_e = sum(r["expected"] for r in rows) / n
    mean_w = sum(r["worst"] for r in rows) / n
    mean_b = sum(r["bound"] for r in rows) / n
    mean_naive = sum(r["naive"] for r in rows) / n

    print()
    print(f"mean expected questions : {mean_e:.2f}")
    print(f"mean worst-case         : {mean_w:.2f}")
    print(f"mean entropy bound      : {mean_b:.2f}")
    print(f"mean naive baseline     : {mean_naive:.2f}")
    print(f"efficiency vs naive     : {mean_naive / mean_e:.2f}x" if mean_e else "")
    print()
    expected = [c["classification_expected"] for c in fixture["cases"]]
    print(f"means / end       : {expected.count('means')} / {expected.count('end')}")
    passed = mean_e <= 2.0 and mean_w <= 3
    print("gate: mean expected <= 2.0 ->", "PASS" if mean_e <= 2.0 else "FAIL")
    print("gate: no case exceeds 3 questions ->", "PASS" if mean_w <= 3 else "FAIL")
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
