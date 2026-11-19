#!/usr/bin/env python3
"""M1 adapter check: fake subject passes, no-op subject fails, through Inspect."""

from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path

try:
    from inspect_ai import eval as inspect_eval
except ImportError:
    print("inspect_ai is not installed; skipping M1 adapter check")
    raise SystemExit(0)

ROOT = Path(__file__).resolve().parent
ADAPTER = ROOT.parent / "adapter.py"
FAKE = ROOT / "fake_subject.sh"
NOOP = ROOT / "noop_subject.sh"
TASKS = [ROOT / "task-command.json", ROOT / "task-artifact.json"]
TRACE_SCHEMA = ROOT.parents[2] / "schemas" / "1ash" / "trace.schema.json"
TRACE_FIXTURE = ROOT / "journal-fixture.jsonl"

sys.path.insert(0, str(ROOT.parent))
from adapter import journal_to_trace  # noqa: E402 - adapter path is local


def run_one(task_file: Path, subject: Path) -> float:
    with tempfile.TemporaryDirectory(prefix="1ash-inspect-log-") as log_dir:
        logs = inspect_eval(
            f"{ADAPTER}@one_ash_task",
            model="mockllm/model",
            task_args={"task_file": str(task_file), "subject": str(subject)},
            display="none",
            log_dir=log_dir,
            log_format="json",
        )
    if len(logs) != 1:
        raise AssertionError(f"expected one eval log, got {len(logs)}")
    log = logs[0]
    if log.status != "success":
        raise AssertionError(f"eval status {log.status}: {log.error}")
    scores = log.results.scores
    if not scores:
        raise AssertionError("no scores")
    metrics = scores[0].metrics
    return float(metrics["accuracy"].value)


def check_trace() -> list[str]:
    trace = journal_to_trace(TRACE_FIXTURE, "m1-trace")
    problems = []
    kinds = [event["type"] for event in trace]
    for required in (
        "run_started",
        "model_request",
        "model_response",
        "tool_call",
        "tool_result",
        "artifact_written",
        "artifact_verified",
        "run_stopped",
    ):
        if required not in kinds:
            problems.append(f"trace is missing {required}")
    if [event["seq"] for event in trace] != list(range(len(trace))):
        problems.append("trace seq is not contiguous from zero")
    try:
        import jsonschema
    except ImportError:
        print("jsonschema not installed; trace structural check only")
        return problems
    schema = json.loads(TRACE_SCHEMA.read_text(encoding="utf-8"))
    validator = jsonschema.Draft202012Validator(schema)
    for event in trace:
        for error in validator.iter_errors(event):
            problems.append(f"{event['type']}: {error.message}")
    return problems


def main() -> int:
    failures = []
    for task_file in TASKS:
        passed = run_one(task_file, FAKE)
        failed = run_one(task_file, NOOP)
        print(json.dumps({"task": task_file.name, "fake": passed, "noop": failed}))
        if passed != 1.0:
            failures.append(f"{task_file.name}: fake subject scored {passed}")
        if failed != 0.0:
            failures.append(f"{task_file.name}: no-op subject scored {failed}")
    trace_failures = check_trace()
    if trace_failures:
        failures.extend(trace_failures)
    else:
        print("1ASH trace conversion passed")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("1ASH M1 adapter passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
