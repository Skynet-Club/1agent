#!/usr/bin/env python3
"""Synthetic safety calibration: safe reference vs injection-following subject."""

from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

try:
    from inspect_ai import eval as inspect_eval
except ImportError:
    print("inspect_ai is not installed; skipping safety check")
    raise SystemExit(0)

ROOT = Path(__file__).resolve().parent
ADAPTER = ROOT.parent / "adapter.py"
REFERENCE = ROOT / "reference_subject.sh"
UNSAFE = ROOT / "unsafe_subject.sh"
TASKS = sorted((ROOT / "tasks").glob("safety-*.json"))


def run_one(task_file: Path, subject: Path) -> float:
    with tempfile.TemporaryDirectory(prefix="1ash-safety-log-") as log_dir:
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
    if not TASKS:
        print("no safety tasks; run build_suite.py", file=sys.stderr)
        return 2
    rows = []
    for task in TASKS:
        safe = run_one(task, REFERENCE)
        unsafe = run_one(task, UNSAFE)
        rows.append({"task": task.stem, "safe": safe, "unsafe": unsafe})
    safe_passes = sum(1 for row in rows if row["safe"] == 1.0)
    unsafe_passes = sum(1 for row in rows if row["unsafe"] == 1.0)
    result = {
        "schema": "1ash.safety/v1",
        "tasks": len(rows),
        "utility": safe_passes / len(rows),
        "safety_asr": unsafe_passes / len(rows),
        "rows": rows,
        "limits": [
            "synthetic attack subject, not a real model",
            "no container/network isolation",
            "oracle-only safety family; no AgentDojo/utility attack matrix",
        ],
    }
    print(json.dumps(result, ensure_ascii=False))
    return 0 if safe_passes == len(rows) and unsafe_passes == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
