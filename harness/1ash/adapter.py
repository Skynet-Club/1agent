"""Minimal 1ASH -> Inspect adapter (M1).

Subject protocol: the subject executable receives
  --task <instruction> --workspace <dir> --journal-dir <dir>
and writes its artifacts into the workspace. The first real subject is
`subject_1agent.sh`; tests use a deterministic fake subject.

This is a local runner skeleton, not the full 1ASH report/scoring pipeline.
"""

from __future__ import annotations

import hashlib
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


def _short(value: object, limit: int = 2_000) -> str:
    text = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)
    return text[:limit]


def _arguments_sha256(arguments: object) -> str:
    encoded = json.dumps(arguments, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def journal_to_trace(journal_path: Path, task_id: str) -> list[dict]:
    """Project a 1Agent run journal onto the closed 1ASH Trace IR."""
    events: list[dict] = []
    run_id = journal_path.stem
    last_answer: dict = {}

    def emit(event_type: str, data: dict[str, object], timestamp: int) -> None:
        events.append(
            {
                "schema": "1ash.trace/v1",
                "run_id": run_id,
                "seq": len(events),
                "timestamp_unix_ms": max(int(timestamp), 0),
                "type": event_type,
                "span_id": f"span-{len(events)}",
                "parent_span_id": None,
                "data": data,
            }
        )

    for line in journal_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        event = record.get("event") or {}
        data = event.get("data") or {}
        timestamp = int(record.get("timestamp_unix_ms") or 0)
        run_id = record.get("run_id") or run_id
        kind = event.get("type")
        if kind == "run_started":
            request = data.get("request") or {}
            emit(
                "run_started",
                {
                    "task_id": task_id,
                    "mode": data.get("mode"),
                    "objective": _short(request.get("objective"), 500),
                    "permissions": request.get("permissions"),
                    "budget": request.get("budget"),
                },
                timestamp,
            )
        elif kind == "model_requested":
            request = data.get("request") or {}
            messages = request.get("messages") or []
            emit(
                "model_request",
                {
                    "mode": request.get("mode"),
                    "message_count": len(messages),
                    "tool_count": len(request.get("tools") or []),
                },
                timestamp,
            )
        elif kind == "model_responded":
            response = data.get("response") or {}
            turn = response.get("turn") or {}
            emit(
                "model_response",
                {
                    "provider_id": response.get("provider_id"),
                    "model": response.get("model"),
                    "turn": turn.get("kind"),
                    "usage": response.get("usage"),
                },
                timestamp,
            )
        elif kind == "tool_called":
            call = data.get("call") or {}
            emit(
                "tool_call",
                {
                    "call_id": call.get("id"),
                    "name": call.get("name"),
                    "arguments_sha256": _arguments_sha256(call.get("arguments")),
                },
                timestamp,
            )
        elif kind == "tool_observed":
            observation = data.get("observation") or {}
            metadata = observation.get("metadata") or {}
            status = observation.get("status")
            error_code = observation.get("error_code")
            emit(
                "tool_result",
                {
                    "call_id": observation.get("call_id"),
                    "tool": observation.get("tool"),
                    "status": status,
                    "error_code": error_code,
                    "summary": _short(observation.get("summary"), 500),
                },
                timestamp,
            )
            if metadata.get("path") and metadata.get("sha256"):
                emit(
                    "artifact_written",
                    {
                        "path": metadata.get("path"),
                        "sha256": metadata.get("sha256"),
                        "bytes": metadata.get("bytes"),
                    },
                    timestamp,
                )
            if observation.get("tool") == "verify_artifact":
                if metadata.get("decision") == "verified":
                    emit(
                        "artifact_verified",
                        {
                            "path": metadata.get("path"),
                            "sha256": metadata.get("sha256"),
                        },
                        timestamp,
                    )
                elif metadata.get("decision") == "failed":
                    emit(
                        "verification_failed",
                        {
                            "path": metadata.get("path"),
                            "checks": _short(observation.get("content"), 500),
                        },
                        timestamp,
                    )
            card_id = metadata.get("approval_card_id") or metadata.get("approval_id")
            if error_code == "approval_required" or card_id:
                emit(
                    "approval_requested",
                    {
                        "tool": observation.get("tool"),
                        "card_id": card_id,
                        "reason": error_code,
                    },
                    timestamp,
                )
            if error_code in {"authority_denied", "permission_denied", "write_not_allowed"}:
                emit(
                    "policy_denied",
                    {
                        "tool": observation.get("tool"),
                        "error_code": error_code,
                        "summary": _short(observation.get("summary"), 500),
                    },
                    timestamp,
                )
        elif kind == "answer_produced":
            last_answer = data.get("answer") or {}
        elif kind == "run_stopped":
            emit(
                "run_stopped",
                {
                    "reason": data.get("reason"),
                    "answer_status": last_answer.get("status"),
                    "conclusion": _short(last_answer.get("conclusion"), 1_000),
                },
                timestamp,
            )
    return events


def write_trace(
    workspace: Path, task_id: str, journal_path: Path | None
) -> tuple[Path | None, str | None]:
    if journal_path is None or not journal_path.is_file():
        return None, "subject produced no readable journal"
    try:
        trace = journal_to_trace(journal_path, task_id)
    except (OSError, ValueError) as error:
        return None, f"cannot read journal: {error}"
    path = workspace / ".1ash" / "trace.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(event, ensure_ascii=False) + "\n" for event in trace),
        encoding="utf-8",
    )
    return path, None


def _journal_from_output(workspace: Path, stdout: str) -> Path | None:
    try:
        result = json.loads(stdout)
    except json.JSONDecodeError:
        result = {}
    candidate = result.get("journal_path")
    if isinstance(candidate, str):
        path = Path(candidate)
        if path.is_file() and path.resolve().is_relative_to(workspace.resolve()):
            return path
    journals = sorted(
        (workspace / ".journals").glob("*.jsonl"),
        key=lambda item: item.stat().st_mtime,
        reverse=True,
    )
    return journals[0] if journals else None


@solver
def one_agent_solver(
    subject: str = "1agent",
    timeout_s: int = 600,
    docker_image: str = "",
    docker_subject: str = "/opt/1ash/subject_1agent.sh",
) -> Solver:
    async def solve(state: TaskState, generate) -> TaskState:
        task = state.metadata["task"]
        workspace = Path(tempfile.mkdtemp(prefix=f"1ash-{task['id']}-"))
        journal_dir = workspace / ".journals"
        journal_dir.mkdir()
        if docker_image:
            command = [
                "docker",
                "run",
                "--rm",
                "--network",
                "none",
                "--cpus",
                "1",
                "--memory",
                "512m",
                "-v",
                f"{workspace}:/workspace",
                "-v",
                f"{journal_dir}:/journal",
                "-w",
                "/workspace",
                docker_image,
                docker_subject,
                "--task",
                task["instruction"],
                "--workspace",
                "/workspace",
                "--journal-dir",
                "/journal",
            ]
        else:
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
                env={
                    **os.environ,
                    "ONEASH_TASK_ID": task["id"],
                    "ONEASH_REFERENCE_DIR": str(
                        Path(__file__).resolve().parent / "suite" / "references"
                    ),
                },
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
        journal_path = _journal_from_output(workspace, process.stdout)
        trace_path, trace_error = write_trace(workspace, task["id"], journal_path)
        state.metadata["journal_path"] = str(journal_path) if journal_path else None
        state.metadata["trace_path"] = str(trace_path) if trace_path else None
        state.metadata["trace_error"] = trace_error
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
def one_ash_task(
    task_file: str,
    subject: str = "1agent",
    timeout_s: int = 600,
    docker_image: str = "",
    docker_subject: str = "/opt/1ash/subject_1agent.sh",
) -> Task:
    task = load_task(task_file)
    sample = Sample(
        input=task["instruction"],
        target=task["id"],
        metadata={"task": task},
    )
    return Task(
        dataset=MemoryDataset([sample], name=task["id"]),
        solver=one_agent_solver(
            subject=subject,
            timeout_s=timeout_s,
            docker_image=docker_image,
            docker_subject=docker_subject,
        ),
        scorer=one_ash_scorer(),
    )
