#!/usr/bin/env python3
"""Research harness: run the reframing pilot against a live provider.

This is measurement code, not product code. It does not change the runtime and
is not wired into the CLI. It answers one mechanical question:

    when the request is a means, does the model ask a splitting question;
    when it is not, does it stay quiet?

Semantic correctness (is the proposed need right? would a user accept it?) is
NOT measured here. That requires external labelers, per
docs/thesis/02-eval-protocol.md.

Usage:
    python3 scripts/reframing/run_pilot.py --limit 1        # smoke
    python3 scripts/reframing/run_pilot.py --reps 3         # full pilot

The API key is read from ~/.pi/agent/auth.json and never printed or written.
"""

import argparse
import hashlib
import json
import os
import statistics
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

FIXTURE = Path(__file__).resolve().parents[2] / "fixtures/reframing/pilot-v0.json"
AUTH = Path.home() / ".pi/agent/auth.json"
DEFAULT_MODEL = "deepseek-flash"
BASE_URL = "https://api.deepseek.com/v1/chat/completions"

# Conservative upper bound used only for the budget guard, not for billing.
USD_PER_M_INPUT = 0.30
USD_PER_M_OUTPUT = 1.20

YES_NO = {"是", "否", "不是", "对", "不对", "会", "不会", "有", "没有", "需要", "不需要"}

PROMPT_VERSION = "v3"

SYSTEM = (Path(__file__).resolve().parents[2] / "prompts/reframe-v3.txt").read_text(encoding="utf-8")


def load_key():
    key = os.environ.get("DEEPSEEK_API_KEY", "").strip()
    if key:
        return key
    with AUTH.open(encoding="utf-8") as handle:
        return json.load(handle)["deepseek"]["key"]


def call(api_key, model, request, constraints, timeout=60):
    payload = {
        "model": model,
        "temperature": 0.7,
        "messages": [
            {"role": "system", "content": SYSTEM},
            {"role": "user", "content": render_user(request, constraints)},
        ],
    }
    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        BASE_URL,
        data=body,
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {api_key}",
        },
        method="POST",
    )
    started = time.time()
    with urllib.request.urlopen(req, timeout=timeout) as response:
        raw = response.read().decode("utf-8")
    elapsed = int((time.time() - started) * 1000)
    data = json.loads(raw)
    text = data["choices"][0]["message"]["content"]
    usage = data.get("usage", {})
    return {
        "text": text,
        "latency_ms": elapsed,
        "prompt_tokens": int(usage.get("prompt_tokens", 0)),
        "completion_tokens": int(usage.get("completion_tokens", 0)),
    }


def render_user(request, constraints):
    lines = [f"请求：{request}"]
    if constraints:
        lines.append("已知约束：" + "；".join(constraints))
    else:
        lines.append("已知约束：无")
    return "\n".join(lines)


def parse(text):
    """Strict JSON parse. A markdown fence is recorded as a format violation."""
    stripped = text.strip()
    fenced = stripped.startswith("```")
    if fenced:
        stripped = stripped.strip("`")
        if stripped.startswith("json"):
            stripped = stripped[4:]
        stripped = stripped.strip()
    try:
        value = json.loads(stripped)
    except json.JSONDecodeError as error:
        return None, {"error": f"json: {error}", "fenced": fenced}
    if not isinstance(value, dict):
        return None, {"error": "not an object", "fenced": fenced}
    return value, {"error": None, "fenced": fenced}


def evaluate(case, parsed):
    """Mechanical checks only. No semantic judgement."""
    expected = case["classification_expected"]
    got = str(parsed.get("classification", "")).strip().lower()
    options = parsed.get("options") or []
    question = parsed.get("question")
    asked = bool(got == "means" and isinstance(question, str) and question.strip() and len(options) >= 2)
    return {
        "expected": expected,
        "got": got,
        "correct_classification": got == expected,
        "asked_question": asked,
        "options": len(options) if isinstance(options, list) else 0,
        "binary": len(options) == 2 and all(str(o).strip() in YES_NO for o in options),
        "over_reframed": expected == "end" and asked,
        "under_reframed": expected == "means" and not asked,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", default=str(FIXTURE))
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--limit", type=int, default=0, help="only first N cases (smoke)")
    parser.add_argument("--max-usd", type=float, default=0.20)
    parser.add_argument("--model", default=os.environ.get("DEEPSEEK_MODEL", DEFAULT_MODEL))
    parser.add_argument("--out", default="")
    args = parser.parse_args()

    fixture = json.loads(Path(args.fixture).read_text(encoding="utf-8"))
    cases = fixture["cases"][: args.limit] if args.limit else fixture["cases"]

    stamp = time.strftime("%Y%m%dT%H%M%SZ", time.gmtime())
    out_dir = Path(args.out) if args.out else (
        Path(os.environ.get("XDG_STATE_HOME", Path.home() / ".local/state")) / "1agent-reframing" / stamp
    )
    out_dir.mkdir(parents=True, exist_ok=True)

    api_key = load_key()
    spent = 0.0
    records = []
    aborted = None

    for case in cases:
        for rep in range(1, args.reps + 1):
            if aborted:
                break
            try:
                result = call(api_key, args.model, case["stated_request"], case.get("constraints") or [])
            except urllib.error.HTTPError as error:
                records.append({"case": case["id"], "rep": rep, "error": f"http {error.code}"})
                continue
            except Exception as error:  # noqa: BLE001 - research harness, record and continue
                records.append({"case": case["id"], "rep": rep, "error": f"{type(error).__name__}: {error}"})
                continue

            cost = (
                result["prompt_tokens"] / 1_000_000 * USD_PER_M_INPUT
                + result["completion_tokens"] / 1_000_000 * USD_PER_M_OUTPUT
            )
            spent += cost

            parsed, format_info = parse(result["text"])
            record = {
                "case": case["id"],
                "family": case.get("family"),
                "rep": rep,
                "latency_ms": result["latency_ms"],
                "prompt_tokens": result["prompt_tokens"],
                "completion_tokens": result["completion_tokens"],
                "cost_usd_estimate": round(cost, 6),
                "fenced": format_info["fenced"],
                "parse_error": format_info["error"],
                "content_sha256": hashlib.sha256(result["text"].encode("utf-8")).hexdigest(),
            }
            if parsed is None:
                record["evaluation"] = None
            else:
                record["evaluation"] = evaluate(case, parsed)
                record["classification"] = parsed.get("classification")
                record["question"] = parsed.get("question")
                record["options"] = parsed.get("options")
                record["hypotheses"] = parsed.get("hypotheses")
                record["substitutes"] = parsed.get("substitutes")
            records.append(record)

            if spent > args.max_usd:
                aborted = f"budget cap reached: {spent:.4f} USD > {args.max_usd}"
                break
        if aborted:
            break

    responses = out_dir / "responses.jsonl"
    with responses.open("w", encoding="utf-8") as handle:
        for record in records:
            handle.write(json.dumps(record, ensure_ascii=False) + "\n")

    ok = [r for r in records if r.get("evaluation")]
    evals = [r["evaluation"] for r in ok]
    parse_failures = sum(1 for r in records if r.get("parse_error"))
    means = [e for e in evals if e["expected"] == "means"]
    ends = [e for e in evals if e["expected"] == "end"]
    latencies = [r["latency_ms"] for r in ok]

    def rate(part, total):
        return round(part / total, 3) if total else None

    summary = {
        "model": args.model,
        "prompt_version": PROMPT_VERSION,
        "prompt_sha256": hashlib.sha256(SYSTEM.encode("utf-8")).hexdigest(),
        "fixture": Path(args.fixture).name,
        "reps": args.reps,
        "calls": len(records),
        "parse_failures": parse_failures,
        "fenced_responses": sum(1 for r in records if r.get("fenced")),
        "classification_accuracy": rate(sum(1 for e in evals if e["correct_classification"]), len(evals)),
        "ask_rate_on_means": rate(sum(1 for e in means if e["asked_question"]), len(means)),
        "over_reframe_rate_on_end": rate(sum(1 for e in ends if e["over_reframed"]), len(ends)),
        "mean_options": round(statistics.mean([e["options"] for e in evals]), 2) if evals else None,
        "binary_question_share": rate(sum(1 for e in evals if e["binary"]), len(evals)),
        "latency_p50_ms": int(statistics.median(latencies)) if latencies else None,
        "latency_p95_ms": sorted(latencies)[int(len(latencies) * 0.95) - 1] if latencies else None,
        "prompt_tokens": sum(r.get("prompt_tokens", 0) for r in records),
        "completion_tokens": sum(r.get("completion_tokens", 0) for r in records),
        "cost_usd_estimate": round(spent, 4),
        "aborted": aborted,
        "responses_sha256": hashlib.sha256(responses.read_bytes()).hexdigest(),
    }
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    print(json.dumps(summary, indent=2, ensure_ascii=False))
    print(f"\nartifacts {out_dir}")

    if parse_failures > len(records) * 0.2:
        print("\nparse failure rate above 20%: the output contract does not hold", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
