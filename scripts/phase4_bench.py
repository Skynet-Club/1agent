#!/usr/bin/env python3
"""Phase 4 three-arm benchmark over evals/real-tasks-v1.json.

Arms:
  full  - 1agent run with the full tool surface
  core7 - 1agent run with the Core 7 Act surface (5 real tools + virtual)
  pi    - Pi 1.5.2 default tools in a locked-down Docker container

All arms use deepseek-flash via http://127.0.0.1:8787/v1. Completion is judged
by neutral workspace/answer checks, not 1agent's structured rubric, so the
three arms are scored the same way. This is a local measurement, not a
leaderboard.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import select
import shutil
import signal
import statistics
import subprocess
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENDPOINT = "http://127.0.0.1:8787/v1"
MODEL = "deepseek/deepseek-flash"
CORE_TOOLS = [
    "read_file",
    "write_file",
    "terminal_create",
    "terminal_exec",
    "acceptance_gate",
]
DOCKER_IMAGE = "debian:bookworm-slim"
TERM_TIMEOUT_S = 240


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_agent_config(path: Path) -> None:
    source = Path.home() / ".1a/config.json"
    config = json.loads(source.read_text(encoding="utf-8"))
    for provider in config.get("providers", []):
        provider["base_url"] = ENDPOINT
    config.setdefault("permissions", {})["allow_network"] = True
    path.write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def prepare_workspace(root: Path, arm: str, run: int, case: dict) -> Path:
    workspace = root / "workspaces" / arm / f"run-{run}" / case["id"]
    if workspace.exists():
        shutil.rmtree(workspace)
    workspace.mkdir(parents=True)
    for item in case.get("setup_files", []):
        target = (workspace / item["path"]).resolve()
        if not target.is_relative_to(workspace.resolve()):
            raise ValueError(f"setup path escapes workspace: {item['path']}")
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(item["content"], encoding="utf-8")
    return workspace


def run_process(argv: list[str], cwd: Path, env: dict[str, str], timeout: int, log_dir: Path) -> tuple[int | None, bool, int, Path, Path]:
    stdout_path = log_dir / "stdout.log"
    stderr_path = log_dir / "stderr.log"
    started = time.monotonic()
    timed_out = False
    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open("w", encoding="utf-8") as stderr:
        process = subprocess.Popen(argv, cwd=cwd, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
            code = None
    return code, timed_out, int((time.monotonic() - started) * 1000), stdout_path, stderr_path


def run_1agent(arm: str, workspace: Path, objective: str, config: Path, run_dir: Path) -> dict:
    run_dir.mkdir(parents=True, exist_ok=True)
    journal_dir = run_dir / "journals"
    journal_dir.mkdir()
    argv = [
        "1agent", "run",
        "--config", str(config),
        "--task", objective,
        "--read-root", str(workspace),
        "--journal-dir", str(journal_dir),
        "--max-steps", "8",
        "--max-tool-calls", "8",
        "--timeout-seconds", str(TERM_TIMEOUT_S - 20),
        "--stream",
        "--json",
    ]
    if arm == "core7":
        for tool in CORE_TOOLS:
            argv.extend(["--allowed-tools", tool])
    if arm == "fast":
        argv.append("--fast")
    env = {**os.environ, "ONEAGENT_SESSION_DIR": str(run_dir / "sessions")}
    if arm == "fast":
        env["ONEAGENT_HOME"] = str(run_dir / "home")
    code, timed_out, wall_ms, stdout_path, stderr_path = run_process(argv, workspace, env, TERM_TIMEOUT_S, run_dir)
    result = {"exit_code": code, "timed_out": timed_out, "wall_ms": wall_ms, "stdout": stdout_path, "stderr": stderr_path}
    try:
        output = json.loads(stdout_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        output = {}
    answer = output.get("answer") or {}
    result["answer_status"] = answer.get("status")
    answer_parts = [answer.get("conclusion") or ""]
    for claim in answer.get("claims") or []:
        answer_parts.append(str(claim.get("text") or ""))
        answer_parts.extend(str(source) for source in claim.get("sources") or [])
    for evidence in answer.get("evidence") or []:
        if isinstance(evidence, dict):
            answer_parts.append(str(evidence.get("source") or evidence.get("path") or ""))
        else:
            answer_parts.append(str(evidence))
    result["answer_text"] = "\n".join(part for part in answer_parts if part)
    journal = Path(output.get("journal_path") or "")
    if not journal.is_file():
        journals = sorted(journal_dir.glob("*.jsonl"), key=lambda p: p.stat().st_mtime, reverse=True)
        journal = journals[0] if journals else Path()
    events = []
    if journal.is_file():
        for line in journal.read_text(encoding="utf-8").splitlines():
            try:
                events.append(json.loads(line).get("event") or {})
            except json.JSONDecodeError:
                continue
    metrics = []
    tool_calls = failed_tools = model_calls = 0
    for event in events:
        kind = event.get("type")
        data = event.get("data") or {}
        if kind == "step_model_metrics":
            metrics.append(data.get("metrics") or {})
        elif kind == "model_responded":
            model_calls += 1
        elif kind == "tool_called":
            tool_calls += 1
        elif kind == "tool_observed":
            if (data.get("observation") or {}).get("status") != "ok":
                failed_tools += 1
    first = metrics[0] if metrics else {}
    ttft = next((m.get("ttft_ms") for m in metrics if isinstance(m.get("ttft_ms"), int)), None)
    result.update(
        {
            "model_calls": model_calls,
            "tool_calls": tool_calls,
            "failed_tool_calls": failed_tools,
            "ttft_ms": ttft,
            "fixed_tokens": (first.get("system_tokens") or 0) + (first.get("tool_schema_tokens") or 0),
            "prompt_tokens": sum(int(m.get("input_tokens") or 0) for m in metrics),
            "completion_tokens": sum(int(m.get("output_tokens") or 0) for m in metrics),
            "cached_tokens": sum(int(m.get("cache_hit_tokens") or 0) for m in metrics),
            "journal": str(journal),
        }
    )
    if arm == "fast":
        accept = subprocess.run(
            ["1agent", "accept", "--queue", "--json"],
            cwd=workspace,
            env=env,
            capture_output=True,
            text=True,
            timeout=60,
        )
        receipt = None
        for line in accept.stdout.splitlines():
            try:
                receipt = json.loads(line)
            except json.JSONDecodeError:
                continue
            if isinstance(receipt, dict):
                break
        if isinstance(receipt, dict):
            result["delivery_status"] = receipt.get("status")
            result["delivery_manifest_sha256"] = receipt.get("manifest_sha256")
        else:
            result["delivery_status"] = None
            result["delivery_error"] = accept.stderr[-500:]
    return result


def pi_argv(workspace: Path, task: str, name: str) -> list[str]:
    node = Path(shutil.which("node") or "").resolve()
    pi_agent = Path.home() / ".pi/agent"
    if not (pi_agent / "auth.json").is_file():
        raise SystemExit("Pi auth.json is missing")
    return [
        "docker", "run", "--rm", "--name", name,
        "--network", "host", "--read-only",
        "--user", f"{os.getuid()}:{os.getgid()}",
        "--cap-drop", "ALL", "--security-opt", "no-new-privileges",
        "-e", "HOME=/home/pirun",
        "-e", f"P4_TASK={task}",
        "-e", "PATH=/config/bin:/usr/local/bin:/usr/bin:/bin",
        "-v", f"{node}:/usr/local/bin/node:ro",
        "-v", f"{pi_agent}:/config:ro",
        "-v", f"{workspace}:{workspace}:rw",
        "-w", str(workspace),
        "--tmpfs", "/home/pirun:rw,exec,nosuid,size=128m",
        "--tmpfs", "/tmp:rw,exec,nosuid,size=256m",
        DOCKER_IMAGE,
        "sh", "-c",
        'mkdir -p "$HOME/.pi/agent" && '
        'cp /config/auth.json /config/settings.json /config/models.json "$HOME/.pi/agent/" && '
        'exec /config/bin/pi --print --mode json --no-session --no-extensions --no-skills '
        '--no-context-files --no-mcp --model deepseek/deepseek-flash "$P4_TASK"',
    ]


def run_pi(workspace: Path, objective: str, run_dir: Path) -> dict:
    run_dir.mkdir(parents=True, exist_ok=True)
    name = f"p4-pi-{uuid.uuid4().hex[:8]}"
    argv = pi_argv(workspace, objective, name)
    stdout_path = run_dir / "stdout.jsonl"
    stderr_path = run_dir / "stderr.log"
    started = time.monotonic()
    timed_out = False
    first_token_ms = None
    assistant_started = None
    lines: list[str] = []
    with stdout_path.open("w", encoding="utf-8") as stdout, stderr_path.open("w", encoding="utf-8") as stderr:
        process = subprocess.Popen(argv, cwd=workspace, stdout=subprocess.PIPE, stderr=stderr, text=True, bufsize=1, start_new_session=True)
        assert process.stdout is not None
        deadline = started + TERM_TIMEOUT_S
        file_descriptor = process.stdout.fileno()
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                timed_out = True
                os.killpg(process.pid, signal.SIGKILL)
                break
            ready, _, _ = select.select([file_descriptor], [], [], min(remaining, 0.5))
            if not ready:
                if process.poll() is not None:
                    break
                continue
            line = process.stdout.readline()
            if not line:
                if process.poll() is not None:
                    break
                continue
            lines.append(line)
            stdout.write(line)
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            kind = event.get("type")
            message = event.get("message") or {}
            if kind == "message_start" and message.get("role") == "assistant":
                assistant_started = time.monotonic()
            elif kind == "message_update" and assistant_started is not None and first_token_ms is None:
                assistant_event = event.get("assistantMessageEvent") or {}
                if assistant_event.get("type") in {"text_start", "text_delta"}:
                    first_token_ms = int((time.monotonic() - assistant_started) * 1000)
            elif kind == "message_end" and message.get("role") == "assistant":
                assistant_started = None
        code = process.wait()
    if timed_out:
        subprocess.run(["docker", "rm", "-f", name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        code = None
    events = []
    for line in lines:
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    model_calls = tool_calls = failed_tools = 0
    prompt_tokens = completion_tokens = cached_tokens = 0
    answer_parts: list[str] = []
    system_text = ""
    for event in events:
        kind = event.get("type")
        message = event.get("message") or {}
        if kind == "message_end" and message.get("role") == "assistant":
            model_calls += 1
            usage = message.get("usage") or {}
            prompt_tokens += int(usage.get("input") or 0) + int(usage.get("cacheRead") or 0) + int(usage.get("cacheWrite") or 0)
            completion_tokens += int(usage.get("output") or 0)
            cached_tokens += int(usage.get("cacheRead") or 0)
            for part in message.get("content") or []:
                if isinstance(part, dict) and part.get("type") == "text":
                    answer_parts.append(part.get("text") or "")
        elif kind == "tool_execution_start":
            tool_calls += 1
        elif kind == "tool_execution_end" and event.get("isError"):
            failed_tools += 1
        elif kind == "agent_end":
            messages = event.get("messages") or []
            if messages and messages[0].get("role") == "system":
                sections = messages[0].get("sections") or {}
                system_text = "\n".join(str(value) for value in sections.values())
    return {
        "exit_code": code,
        "timed_out": timed_out,
        "wall_ms": int((time.monotonic() - started) * 1000),
        "stdout": stdout_path,
        "stderr": stderr_path,
        "answer_status": None,
        "answer_text": "".join(answer_parts),
        "model_calls": model_calls,
        "tool_calls": tool_calls,
        "failed_tool_calls": failed_tools,
        "ttft_ms": first_token_ms,
        "fixed_tokens": (len(system_text) + 3) // 4 if system_text else None,
        "prompt_tokens": prompt_tokens,
        "completion_tokens": completion_tokens,
        "cached_tokens": cached_tokens,
        "journal": "",
    }


def check_case(case_id: str, workspace: Path, answer: str) -> tuple[bool, dict[str, bool]]:
    text = answer.lower()
    checks: dict[str, bool] = {}

    def read(name: str) -> str:
        path = workspace / name
        return path.read_text(encoding="utf-8", errors="replace") if path.is_file() else ""

    if case_id == "bug-fix-last-index":
        source = read("counter.rs")
        checks["uses_saturating_sub"] = "saturating_sub" in source
        checks["returns_last_index"] = bool(re.search(r"saturating_sub\s*\(\s*1\s*\)", source))
    elif case_id == "config-retries":
        checks["retries_is_3"] = bool(re.search(r"(?m)^\s*retries\s*=\s*3\s*$", read("agent.conf")))
    elif case_id == "data-processing-sales":
        summary = read("summary.txt")
        checks["total_60"] = "60" in summary
        checks["records_3"] = bool(re.search(r"\b3\b", summary))
    elif case_id == "research-compare":
        checks["cites_a"] = "a.md" in text
        checks["cites_b"] = "b.md" in text
        checks["compares"] = any(term in text for term in ("cost", "risk", "成本", "风险"))
    elif case_id == "long-task-plan":
        plan = read("plan.md")
        steps = re.findall(
            r"(?im)^\s*(?:#{1,6}\s*)?(?:step\s*[123]\b|\d+[.)]\s+\S|[-*]\s+\S)",
            plan,
        )
        checks["plan_written"] = len(steps) >= 3
        checks["cites_readme"] = "readme.md" in text or "readme.md" in plan.lower()
    elif case_id == "broken-json":
        checks["cites_file"] = "broken.json" in text
        checks["names_invalidity"] = any(
            term in text
            for term in ("trailing comma", "尾逗号", "invalid", "无效", "syntax error", "语法错误")
        )
        checks["no_invented_parse"] = "parsed value" not in text or "not" in text
    elif case_id == "checksum-file":
        content = read("checksum.txt").encode("utf-8")
        expected = hashlib.sha256(b"evidence").hexdigest()
        checks["exact_bytes"] = content == b"evidence"
        checks["reports_hash"] = expected in text
    elif case_id == "directory-summary":
        checks["topic"] = "retry" in text
        checks["cites_a"] = "a.txt" in text
        checks["cites_b"] = "b.txt" in text
    else:
        checks["known_case"] = False
    return all(checks.values()), checks


def aggregate(records: list[dict]) -> dict:
    summary: dict[str, dict] = {}
    for arm in ("full", "core7", "fast", "pi"):
        rows = [row for row in records if row["arm"] == arm]
        if not rows:
            continue

        def median(key: str):
            values = [row[key] for row in rows if isinstance(row.get(key), (int, float))]
            return statistics.median(values) if values else None

        status_met = sum(1 for row in rows if row.get("status_met") is True)
        summary[arm] = {
            "runs": len(rows),
            "neutral_passed": sum(1 for row in rows if row["success"]),
            "neutral_completion_bps": round(10000 * sum(1 for row in rows if row["success"]) / len(rows)),
            "status_met": status_met,
            "status_met_bps": round(10000 * status_met / len(rows)) if arm != "pi" else None,
            "model_calls_median": median("model_calls"),
            "tool_calls_median": median("tool_calls"),
            "failed_tool_calls_median": median("failed_tool_calls"),
            "wall_ms_median": median("wall_ms"),
            "ttft_ms_median": median("ttft_ms"),
            "fixed_tokens_median": median("fixed_tokens"),
            "prompt_tokens_median": median("prompt_tokens"),
            "completion_tokens_median": median("completion_tokens"),
        }
    return summary


def write_summary(out: Path, result: dict, suite: dict, cases: list[dict], arms: list[str]) -> list[str]:
    summary = result["summary"]
    lines = [
        "# Phase 4 — three-arm real-task measurement",
        "",
        f"- Suite: `{suite.get('name')}` ({len(cases)} cases), N={result['runs']}",
        f"- Model: `{MODEL}` via `{ENDPOINT}`",
        f"- Started: {result['started_at']}",
        f"- Finished: {result['finished_at']}",
        f"- 1agent: `{result['binary']['1agent']}` sha256 `{result['binary']['1agent_sha256'][:12]}…`",
        f"- Pi: `{result['binary']['pi']}`",
        "",
        "Completion is neutral (workspace files plus cited answer terms), not 1agent's structured rubric.",
        "User corrections are not measured: every run is non-interactive.",
        "Pi has no structured delivery status; its `1agent status met` cell is not applicable.",
        "1agent status met compares the answer status with the suite's expected status (research-compare expects conditional).",
        "For the fast arm, status met uses the post-run acceptance receipt (`accepted` or expected `conditional`).",
        "1agent fixed tokens come from step-0 metrics; Pi fixed tokens are estimated from its system prompt.",
        "",
        "| Arm | Neutral completion | 1agent status met | Model calls (median) | Tool calls | Wall ms | TTFT ms | Fixed tokens | Prompt tokens | Output tokens |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    for arm in arms:
        row = summary.get(arm)
        if not row:
            continue
        status_met = "—" if arm == "pi" else f"{row['status_met']}/{row['runs']}"
        lines.append(
            f"| {arm} | {row['neutral_passed']}/{row['runs']} | "
            f"{status_met} | {row['model_calls_median']} | {row['tool_calls_median']} | "
            f"{row['wall_ms_median']} | {row['ttft_ms_median']} | {row['fixed_tokens_median']} | "
            f"{row['prompt_tokens_median']} | {row['completion_tokens_median']} |"
        )
    lines.extend(["", "## Per-case neutral completion", "", "| Case | " + " | ".join(arms) + " |", "|---|" + "---|" * len(arms)])
    for case in cases:
        cells = []
        for arm in arms:
            rows = [row for row in result["records"] if row["arm"] == arm and row["case"] == case["id"]]
            cells.append(f"{sum(1 for row in rows if row['success'])}/{len(rows)}")
        lines.append(f"| {case['id']} | " + " | ".join(cells) + " |")
    (out / "summary.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return lines


def rescore(out: Path, suite: dict) -> int:
    result = json.loads((out / "results.json").read_text(encoding="utf-8"))
    cases = {case["id"]: case for case in suite["cases"]}
    for record in result["records"]:
        workspace = out / "workspaces" / record["arm"] / f"run-{record['run']}" / record["case"]
        success, checks = check_case(record["case"], workspace, record.get("answer_text") or "")
        record["success"] = success
        record["checks"] = checks
        expected = cases[record["case"]]["expect"]["answer_status"]
        record["expected_status"] = expected
        record["status_met"] = (
            None
            if record["arm"] == "pi"
            else record.get("answer_status") == expected
        )
    result["summary"] = aggregate(result["records"])
    result["rescored_at"] = datetime.now(timezone.utc).isoformat()
    (out / "results.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    recorded = {record["case"] for record in result["records"]}
    cases = [case for case in suite["cases"] if case["id"] in recorded]
    lines = write_summary(out, result, suite, cases, result["arms"])
    print("\n".join(lines))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", type=Path, default=ROOT / "evals/real-tasks-v1.json")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--arms", default="full,core7,pi")
    parser.add_argument("--cases", default="")
    parser.add_argument("--rescore", action="store_true", help="re-score an existing --out without model calls")
    args = parser.parse_args()

    suite = json.loads(args.suite.read_text(encoding="utf-8"))
    cases = suite["cases"]
    if args.cases:
        wanted = {name.strip() for name in args.cases.split(",") if name.strip()}
        cases = [case for case in cases if case["id"] in wanted]
    arms = [name.strip() for name in args.arms.split(",") if name.strip()]
    for arm in arms:
        if arm not in {"full", "core7", "fast", "pi"}:
            raise SystemExit(f"unknown arm: {arm}")

    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=True)
    if args.rescore:
        return rescore(out, suite)
    config = out / "1agent-8787.json"
    write_agent_config(config)
    records: list[dict] = []
    started_at = datetime.now(timezone.utc).isoformat()

    for run in range(1, args.runs + 1):
        for case in cases:
            objective = case["objective"].replace("${workspace}", "{workspace}")
            for arm in arms:
                workspace = prepare_workspace(out, arm, run, case)
                task = objective.replace("{workspace}", str(workspace))
                run_dir = out / "runs" / arm / case["id"] / f"run-{run}"
                if run_dir.exists():
                    shutil.rmtree(run_dir)
                run_dir.mkdir(parents=True)
                if arm == "pi":
                    result = run_pi(workspace, task, run_dir)
                else:
                    result = run_1agent(arm, workspace, task, config, run_dir)
                success, checks = check_case(case["id"], workspace, result["answer_text"])
                expected_status = case["expect"]["answer_status"]
                if arm == "fast":
                    delivery = result.get("delivery_status")
                    status_met = (
                        delivery == "accepted"
                        if expected_status == "completed"
                        else delivery in {"accepted", "conditional"}
                    )
                elif arm == "pi":
                    status_met = None
                else:
                    status_met = result["answer_status"] == expected_status
                record = {
                    "arm": arm,
                    "case": case["id"],
                    "run": run,
                    "success": success,
                    "checks": checks,
                    "expected_status": expected_status,
                    "status_met": status_met,
                    **{key: value for key, value in result.items() if key not in {"stdout", "stderr"}},
                    "stdout_path": str(result["stdout"]),
                    "stderr_path": str(result["stderr"]),
                }
                records.append(record)
                print(
                    f"[{arm:5}] run {run} {case['id']}: {'PASS' if success else 'FAIL'} "
                    f"status={record.get('answer_status') or '-'} calls={record.get('model_calls')} "
                    f"tools={record.get('tool_calls')} wall={record.get('wall_ms')}ms "
                    f"ttft={record.get('ttft_ms')} fixed={record.get('fixed_tokens')}",
                    flush=True,
                )
                (run_dir / "result.json").write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    summary = aggregate(records)
    result = {
        "suite": suite.get("name"),
        "suite_sha256": sha256(args.suite),
        "model": MODEL,
        "endpoint": ENDPOINT,
        "runs": args.runs,
        "arms": arms,
        "started_at": started_at,
        "finished_at": datetime.now(timezone.utc).isoformat(),
        "binary": {
            "1agent": shutil.which("1agent"),
            "1agent_sha256": sha256(Path(shutil.which("1agent"))),
            "pi": subprocess.run(["pi", "--version"], capture_output=True, text=True, check=False).stdout.strip(),
        },
        "summary": summary,
        "records": records,
    }
    (out / "results.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    lines = write_summary(out, result, suite, cases, arms)
    print("\n" + "\n".join(lines))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
