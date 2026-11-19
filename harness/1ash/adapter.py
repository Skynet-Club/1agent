"""Minimal 1ASH -> Inspect adapter (M1).

Subject protocol: the subject executable receives
  --task <instruction> --workspace <dir> --journal-dir <dir>
and writes its artifacts into the workspace. The first real subject is
`subject_1agent.sh`; tests use a deterministic fake subject.

This is a local runner skeleton, not the full 1ASH report/scoring pipeline.
"""

from __future__ import annotations

import json
import os
import subprocess
import tempfile
from pathlib import Path

from inspect_ai import Task, task
from inspect_ai.dataset import MemoryDataset, Sample
from inspect_ai.model import ModelOutput
from inspect_ai.scorer import Score, Scorer, Target, accuracy, scorer
from inspect_ai.solver import Solver, TaskState, solver

MAX_CAPTURE = 20_000


def load_task(path: str) -> dict:
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if data.get("schema") != "1ash.task/v1":
        raise ValueError(f"unsupported task schema: {data.get('schema')}")
    for key in ("id", "instruction", "environment", "budget", "success"):
        if key not in data:
            raise ValueError(f"task is missing {key}")
    return data


def _workspace_path(workspace: Path, value: str) -> Path:
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError(f"artifact path escapes workspace: {value}")
    resolved = (workspace / path).resolve()
    if not resolved.is_relative_to(workspace.resolve()):
        raise ValueError(f"artifact path escapes workspace: {value}")
    return resolved


@solver
def one_agent_solver(subject: str = "1agent", timeout_s: int = 600) -> Solver:
    async def solve(state: TaskState, generate) -> TaskState:
        task = state.metadata["task"]
        workspace = Path(tempfile.mkdtemp(prefix=f"1ash-{task['id']}-"))
        journal_dir = workspace / ".journals"
        journal_dir.mkdir()
        command = [
            subject,
            "--task",
            task["instruction"],
            "--workspace",
            str(workspace),
            "--journal-dir",
            str(journal_dir),
        ]
        try:
            process = subprocess.run(
                command,
                cwd=workspace,
                capture_output=True,
                text=True,
                timeout=timeout_s,
                check=False,
            )
        except subprocess.TimeoutExpired as error:
            state.metadata["workspace"] = str(workspace)
            state.metadata["subject_exit"] = None
            state.metadata["subject_error"] = f"timeout after {timeout_s}s: {error}"
            state.output = ModelOutput.from_content(
                model="subject", content="subject timed out"
            )
            return state
        state.metadata["workspace"] = str(workspace)
        state.metadata["subject_exit"] = process.returncode
        state.metadata["subject_stdout"] = process.stdout[-MAX_CAPTURE:]
        state.metadata["subject_stderr"] = process.stderr[-MAX_CAPTURE:]
        state.output = ModelOutput.from_content(
            model="subject",
            content=(process.stdout.strip() or process.stderr.strip() or "(no output)")[
                :MAX_CAPTURE
            ],
        )
        return state

    return solve


@scorer(metrics=[accuracy()])
def one_ash_scorer() -> Scorer:
    async def score(state: TaskState, target: Target) -> Score:
        task = state.metadata["task"]
        workspace_value = state.metadata.get("workspace")
        if not workspace_value:
            return Score(value=0.0, explanation="subject produced no workspace")
        workspace = Path(workspace_value)
        gates = task.get("safety", {}).get("gates", [])
        if gates:
            return Score(
                value=0.0,
                explanation=f"safety gates are not implemented in M1: {gates}",
            )
        if state.metadata.get("subject_exit") != 0:
            return Score(
                value=0.0,
                explanation=f"subject exit {state.metadata.get('subject_exit')}: "
                f"{state.metadata.get('subject_stderr', '')[-500:]}",
            )

        total = 0.0
        details = []
        for module in task["success"]["modules"]:
            module_type = module.get("type")
            passed = False
            if module_type == "command":
                try:
                    result = subprocess.run(
                        module["command"],
                        cwd=workspace,
                        shell=True,
                        capture_output=True,
                        text=True,
                        timeout=120,
                        check=False,
                    )
                    passed = result.returncode == 0
                    detail = f"{module['id']}: command exit {result.returncode}"
                except Exception as error:  # noqa: BLE001 - report every checker failure
                    detail = f"{module['id']}: command error {error}"
            elif module_type == "artifact":
                try:
                    path = _workspace_path(workspace, module["path"])
                    passed = path.is_file() and path.stat().st_size > 0
                    detail = f"{module['id']}: artifact {path} exists={passed}"
                except Exception as error:  # noqa: BLE001
                    detail = f"{module['id']}: artifact error {error}"
            else:
                detail = f"{module['id']}: unsupported module type {module_type}"
            if passed:
                total += float(module["weight"])
            details.append(detail)
        return Score(value=round(total, 6), explanation="; ".join(details))

    return score


@task
def one_ash_task(task_file: str, subject: str = "1agent", timeout_s: int = 600) -> Task:
    task = load_task(task_file)
    sample = Sample(
        input=task["instruction"],
        target=task["id"],
        metadata={"task": task},
    )
    return Task(
        dataset=MemoryDataset([sample], name=task["id"]),
        solver=one_agent_solver(subject=subject, timeout_s=timeout_s),
        scorer=one_ash_scorer(),
    )
