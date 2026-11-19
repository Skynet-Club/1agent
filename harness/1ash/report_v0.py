#!/usr/bin/env python3
"""Generate an M1 1ASH report v0 from one Inspect eval log.

This is a local evidence artifact, not a benchmark result. It records one
synthetic task, one subject run, the real score, latency and trace digest, and
keeps every unmeasured field explicit in `limitations`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import subprocess
from datetime import datetime, timezone
from pathlib import Path

DEFAULT_CONFIG = Path.home() / ".1a" / "config.json"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def canonical_digest(value: dict) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return sha256_bytes(encoded.encode("utf-8"))


def cost_micros(journal_path: Path | None) -> int:
    if not journal_path or not journal_path.is_file():
        return 0
    total = 0
    for line in journal_path.read_text(encoding="utf-8").splitlines():
        try:
            event = json.loads(line).get("event") or {}
        except json.JSONDecodeError:
            continue
        if event.get("type") != "model_responded":
            continue
        usage = (event.get("data") or {}).get("response", {}).get("usage") or {}
        total += int(usage.get("cost_micros") or 0)
    return total


def trace_provider(trace_path: Path | None) -> tuple[str, str]:
    if trace_path and trace_path.is_file():
        for line in trace_path.read_text(encoding="utf-8").splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get("type") == "model_response":
                data = event.get("data") or {}
                return str(data.get("provider_id") or "unknown"), str(
                    data.get("model") or "unknown"
                )
    return "unknown", "unknown"


def agent_version(binary: str) -> str:
    try:
        result = subprocess.run(
            [binary, "--version"],
            capture_output=True,
            text=True,
            timeout=10,
            check=False,
        )
        text = (result.stdout or result.stderr).strip()
        return text.splitlines()[0] if text else "unknown"
    except (OSError, subprocess.TimeoutExpired):
        return "unknown"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--task", required=True, type=Path)
    parser.add_argument("--eval-log", required=True, type=Path)
    parser.add_argument("--trace", type=Path)
    parser.add_argument("--config", type=Path, default=DEFAULT_CONFIG)
    parser.add_argument("--agent-binary", default="1agent")
    parser.add_argument("--agent-version")
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    task_bytes = args.task.read_bytes()
    task = json.loads(task_bytes)
    eval_bytes = args.eval_log.read_bytes()
    log = json.loads(eval_bytes)
    sample = log["samples"][0]
    score = float(log["results"]["scores"][0]["metrics"]["accuracy"]["value"])
    metadata = sample.get("metadata") or {}
    trace_path = args.trace or (
        Path(metadata["trace_path"]) if metadata.get("trace_path") else None
    )
    journal_path = (
        Path(metadata["journal_path"]) if metadata.get("journal_path") else None
    )
    provider, model = trace_provider(trace_path)
    config_digest = (
        "sha256:" + sha256_bytes(args.config.read_bytes())
        if args.config.is_file()
        else "missing"
    )
    trace_digest = (
        "sha256:" + sha256_bytes(trace_path.read_bytes())
        if trace_path and trace_path.is_file()
        else "missing"
    )
    total_cost_micros = cost_micros(journal_path)
    duration_ms = int(round(float(sample.get("working_time") or 0.0) * 1000))
    agent = args.agent_version or agent_version(args.agent_binary)
    now = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    task_id = task["id"]
    report = {
        "schema": "1ash.report/v1",
        "report_id": f"1ash-m1-{task_id}-{int(datetime.now().timestamp())}",
        "generated_at": now,
        "harness": {"name": "1ASH", "version": "0.1.0-m1"},
        "task_set": {
            "name": f"1ash-m1-{task_id}",
            "version": task.get("version", "1.0.0"),
            "digest": "sha256:" + sha256_bytes(task_bytes),
        },
        "subject": {
            "agent": "1agent",
            "version": agent,
            "provider": provider,
            "model": model,
            "model_snapshot": model,
            "config_digest": config_digest,
        },
        "environment": {
            "image_digest": f"not-enforced:{task['environment']['image']}",
            "network": "host",
            "tool_versions": {"python": platform.python_version()},
        },
        "metrics": {
            "pass_at_1": score,
            "pass_at_k": score,
            "pass_power_k": score,
            "mean_score": score,
            "harness_score": score,
            "mean_cost_usd": total_cost_micros / 1_000_000,
            "p95_latency_ms": duration_ms,
            "safety_asr": 0.0,
            "confidence_intervals": {},
        },
        "failure_class_counts": {} if score >= 1.0 else {"task_failed": 1},
        "harness_findings": [],
        "limitations": [
            "single synthetic M1 task; not a benchmark or general capability claim",
            "no container isolation; the task image was not used",
            "no safety suite; safety_asr is not a measurement",
            "pass@k and pass^k are recorded at k=1 only",
            "cost is the provider-reported journal cost; unknown is not estimated",
        ],
        "abc_checklist": {},
        "runs": [
            {
                "task_id": task_id,
                "score": score,
                "subject_exit": metadata.get("subject_exit"),
                "duration_ms": duration_ms,
                "eval_log_sha256": "sha256:" + sha256_bytes(eval_bytes),
                "trace_sha256": trace_digest,
                "journal_sha256": (
                    "sha256:" + sha256_bytes(journal_path.read_bytes())
                    if journal_path and journal_path.is_file()
                    else "missing"
                ),
                "cost_usd": total_cost_micros / 1_000_000,
            }
        ],
        "signatures": [],
        "hash_chain_tip": "",
    }
    body = dict(report)
    body.pop("hash_chain_tip", None)
    report["hash_chain_tip"] = "sha256:" + canonical_digest(body)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
