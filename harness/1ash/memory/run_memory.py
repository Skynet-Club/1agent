#!/usr/bin/env python3
"""Run the synthetic 1ASH memory family: reference must pass, no-op must fail.

This validates the harness shape (seed -> read -> act -> score), not 1Agent's
MemoryService. LongMemEval/PersonaMem/MemoryArena remain external benchmarks.
"""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

try:
    from inspect_ai import eval as inspect_eval
except ImportError:
    print("inspect_ai is not installed; skipping memory family")
    raise SystemExit(0)

ROOT = Path(__file__).resolve().parent
ADAPTER = ROOT.parent / "adapter.py"
REFERENCE = ROOT / "reference_subject.sh"
NOOP = ROOT.parent / "tests" / "noop_subject.sh"
TASKS = sorted((ROOT / "tasks").glob("*.json"))


def run_one(task_file: Path, subject: Path) -> float:
    with tempfile.TemporaryDirectory(prefix="1ash-memory-log-") as log_dir:
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
        return 0.0
    return float(log.results.scores[0].metrics["accuracy"].value)


def main() -> int:
    if len(TASKS) != 5:
        print(f"expected 5 memory tasks, found {len(TASKS)}", file=sys.stderr)
        return 2
    failures = []
    rows = []
    for task_file in TASKS:
        reference = run_one(task_file, REFERENCE)
        noop = run_one(task_file, NOOP)
        rows.append({"task": task_file.stem, "reference": reference, "noop": noop})
        if reference != 1.0:
            failures.append(f"{task_file.stem}: reference={reference}")
        if noop != 0.0:
            failures.append(f"{task_file.stem}: noop={noop}")
    print(json.dumps({"tasks": len(rows), "results": rows}, ensure_ascii=False))
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("1ASH memory family passed: reference 5/5, no-op 0/5")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
