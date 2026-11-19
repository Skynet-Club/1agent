#!/usr/bin/env python3
"""Score a suite result into the 1ASH report v0 schema.

This scores the synthetic reference/no-op calibration, not a model subject.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
from datetime import datetime, timezone
from pathlib import Path


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def canonical_digest(value: dict) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def wilson(successes: int, total: int) -> dict[str, float]:
    if total == 0:
        return {"lower": 0.0, "upper": 0.0}
    z = 1.959963984540054
    p = successes / total
    denom = 1 + z * z / total
    centre = (p + z * z / (2 * total)) / denom
    margin = z * ((p * (1 - p) / total + z * z / (4 * total * total)) ** 0.5) / denom
    return {"lower": max(0.0, centre - margin), "upper": min(1.0, centre + margin)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", required=True, type=Path)
    parser.add_argument("--task-dir", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--subject", default="1ash-reference")
    args = parser.parse_args()

    result = json.loads(args.results.read_text(encoding="utf-8"))
    rows = result["results"]
    scores = [float(row["reference"]) for row in rows]
    passed = sum(1 for score in scores if score >= 1.0)
    mean_score = sum(scores) / len(scores) if scores else 0.0
    task_text = "\n".join(
        f"{path.name}:{sha256_text(path.read_text(encoding='utf-8'))}"
        for path in sorted(args.task_dir.glob("*.json"))
    )
    interval = wilson(passed, len(scores))
    report = {
        "schema": "1ash.report/v1",
        "report_id": f"1ash-m2-{datetime.now().strftime('%Y%m%d%H%M%S')}",
        "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "harness": {"name": "1ASH", "version": "0.1.0-m2"},
        "task_set": {
            "name": "1ash-m2-synthetic",
            "version": "1.0.0",
            "digest": "sha256:" + sha256_text(task_text),
        },
        "subject": {
            "agent": args.subject,
            "version": "synthetic",
            "provider": "none",
            "model": "reference",
            "model_snapshot": "reference",
            "config_digest": "synthetic",
        },
        "environment": {
            "image_digest": "not-enforced:synthetic-suite",
            "network": "none",
            "tool_versions": {"python": platform.python_version()},
        },
        "metrics": {
            "pass_at_1": passed / len(scores) if scores else 0.0,
            "pass_at_k": passed / len(scores) if scores else 0.0,
            "pass_power_k": passed / len(scores) if scores else 0.0,
            "mean_score": mean_score,
            "harness_score": mean_score,
            "mean_cost_usd": 0.0,
            "p95_latency_ms": 0,
            "safety_asr": 0.0,
            "confidence_intervals": {"pass_at_1": interval},
        },
        "failure_class_counts": {},
        "harness_findings": [
            {
                "finding": "reference solves every synthetic task; no-op solves none",
                "kind": "calibration",
            }
        ],
        "limitations": [
            "synthetic M2 scaffold; reviewers are role placeholders, not humans",
            "reference subject is not a model; this is not a capability result",
            "human_time_minutes is unmeasured (0)",
            "no container/network isolation; no external pilot",
            "safety_asr is a placeholder; the safety family is oracle-only",
            "cost and latency are not measured for the reference subject",
        ],
        "abc_checklist": {
            "A": "not assessed",
            "B": "not assessed",
            "C": "not assessed",
        },
        "runs": [
            {
                "task_id": row["task"],
                "score": row["reference"],
                "noop": row["noop"],
            }
            for row in rows
        ],
        "signatures": [],
        "hash_chain_tip": "",
    }
    body = dict(report)
    body.pop("hash_chain_tip", None)
    report["hash_chain_tip"] = "sha256:" + canonical_digest(body)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
