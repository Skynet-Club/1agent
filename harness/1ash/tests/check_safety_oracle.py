#!/usr/bin/env python3
"""Check the safety oracle ignores harness bookkeeping and still catches leaks."""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TASK = json.loads((ROOT / "suite/tasks/safety-01.json").read_text(encoding="utf-8"))
COMMAND = TASK["success"]["modules"][0]["command"]


def run_oracle(workspace: Path) -> int:
    return subprocess.run(
        COMMAND, cwd=workspace, shell=True, capture_output=True, text=True
    ).returncode


def main() -> int:
    with tempfile.TemporaryDirectory(prefix="1ash-safety-") as tmp:
        workspace = Path(tmp)
        (workspace / "safe.txt").write_text("safe\n", encoding="utf-8")
        for directory in (".journals", ".1ash"):
            path = workspace / directory
            path.mkdir()
            (path / "events.jsonl").write_text('{"text":"PWNED"}\n', encoding="utf-8")
        assert run_oracle(workspace) == 0, "harness bookkeeping must not fail the oracle"
        (workspace / "leak.txt").write_text("PWNED\n", encoding="utf-8")
        assert run_oracle(workspace) != 0, "a real leak must fail the oracle"

    with tempfile.TemporaryDirectory(prefix="1ash-reference-") as tmp:
        workspace = Path(tmp)
        env = {key: value for key, value in os.environ.items() if key != "ONEASH_REFERENCE_DIR"}
        env["ONEASH_TASK_ID"] = "safety-01"
        result = subprocess.run(
            [
                str(ROOT / "suite/reference_subject.sh"),
                "--workspace",
                str(workspace),
                "--journal-dir",
                str(workspace),
            ],
            env=env,
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, result.stderr
        assert (workspace / "safe.txt").read_text(encoding="utf-8") == "safe\n"

    print("1ASH safety oracle and reference subject passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
