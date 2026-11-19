#!/usr/bin/env python3
"""Run the fake/no-op M1 checks inside the local Docker image, network off."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    from inspect_ai import eval as inspect_eval
except ImportError:
    print("inspect_ai is not installed; skipping Docker M1 check")
    raise SystemExit(0)

ROOT = Path(__file__).resolve().parent
ADAPTER = ROOT.parent / "adapter.py"
TASKS = [ROOT / "task-command.json", ROOT / "task-artifact.json"]
IMAGE = "1ash-1agent:local"


def run_one(task_file: Path, subject: str) -> float:
    with tempfile.TemporaryDirectory(prefix="1ash-docker-log-") as log_dir:
        logs = inspect_eval(
            f"{ADAPTER}@one_ash_task",
            model="mockllm/model",
            task_args={
                "task_file": str(task_file),
                "docker_image": IMAGE,
                "docker_subject": subject,
            },
            display="none",
            log_dir=log_dir,
            log_format="json",
        )
    log = logs[0]
    if log.status != "success":
        raise AssertionError(f"eval status {log.status}: {log.error}")
    return float(log.results.scores[0].metrics["accuracy"].value)


def main() -> int:
    if subprocess.run(["docker", "image", "inspect", IMAGE], capture_output=True).returncode:
        print(f"missing Docker image {IMAGE}; run harness/1ash/build_docker.sh", file=sys.stderr)
        return 2
    failures = []
    for task_file in TASKS:
        fake = run_one(task_file, "/opt/1ash/fake_subject.sh")
        noop = run_one(task_file, "/opt/1ash/noop_subject.sh")
        print(json.dumps({"task": task_file.name, "fake": fake, "noop": noop, "network": "none"}))
        if fake != 1.0:
            failures.append(f"{task_file.name}: fake={fake}")
        if noop != 0.0:
            failures.append(f"{task_file.name}: noop={noop}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("1ASH Docker M1 sandbox passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
