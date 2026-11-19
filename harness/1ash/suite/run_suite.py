#!/usr/bin/env python3
"""Run the synthetic 30-task M2 suite: reference must pass, no-op must fail."""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

try:
    from inspect_ai import eval as inspect_eval
except ImportError:
    print("inspect_ai is not installed; skipping suite")
    raise SystemExit(0)

ROOT = Path(__file__).resolve().parent
ADAPTER = ROOT.parent / "adapter.py"
REFERENCE = ROOT / "reference_subject.sh"
NOOP = ROOT.parent / "tests" / "noop_subject.sh"
TASKS = sorted((ROOT / "tasks").glob("*.json"))


def run_one(task_file: Path, subject: Path) -> tuple[float, str]:
    with tempfile.TemporaryDirectory(prefix="1ash-suite-log-") as log_dir:
        logs = inspect_eval(
            f"{ADAPTER}@one_ash_task",
            model="mockllm/model",
            task_args={"task_file": str(task_file), "subject": str(subject)},
            display="none",
            log_dir=log_dir,
            log_format="json",
        )
    log = logs[0]
    if log.status != "success":
        return 0.0, f"eval {log.status}: {log.error}"
    score = float(log.results.scores[0].metrics["accuracy"].value)
    explanation = ""
    if log.samples:
        score_value = (log.samples[0].scores or {}).get("one_ash_scorer")
        explanation = str(getattr(score_value, "explanation", "") or "")
    return score, explanation


def main() -> int:
    if not TASKS:
        print("no tasks; run build_suite.py", file=sys.stderr)
        return 2
    failures = []
    rows = []
    for task_file in TASKS:
        reference, ref_detail = run_one(task_file, REFERENCE)
        noop, noop_detail = run_one(task_file, NOOP)
        rows.append(
            {
                "task": task_file.stem,
                "reference": reference,
                "noop": noop,
            }
        )
        if reference != 1.0:
            failures.append(f"{task_file.stem}: reference={reference} {ref_detail[-300:]}")
        if noop != 0.0:
            failures.append(f"{task_file.stem}: noop={noop} {noop_detail[-300:]}")
    print(json.dumps({"tasks": len(rows), "results": rows}, ensure_ascii=False))
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("1ASH M2 synthetic suite passed: reference 30/30, no-op 0/30")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
