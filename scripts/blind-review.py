#!/usr/bin/env python3
"""Blind-review pipeline for the fixed golden set (M19-M23).

    scripts/blind-review.py collect --runner 1agent --out answers-1agent.jsonl
    scripts/blind-review.py collect --runner hermes --out answers-hermes.jsonl
    scripts/blind-review.py packet --oneagent answers-1agent.jsonl \
        --baseline answers-hermes.jsonl --out-dir blind-review-2026-11-12 \
        --mapping ~/.1a/blind-review/mapping.json
    scripts/blind-review.py finalize --labels labels.jsonl --out answer-quality.jsonl

`collect` invokes the local `1agent` launcher or
`hermes --safe-mode --provider <provider> --model <model> -z`. Safe mode
ignores the user profile, so the provider and model must be explicit. `packet`
anonymizes A/B with a deterministic seed and writes a reviewer packet; the
sealed mapping is written outside the packet directory. `finalize` strips the
reviewer ids and writes the bare sample shape accepted by
`1agent answer-quality`.
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ANSI = re.compile(r"\x1b\[[0-9;?]*[ -/]*[@-~]")
CONTROL = re.compile(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]")


def clean(text):
    text = ANSI.sub("", text)
    text = CONTROL.sub("", text)
    return text.strip()[:20_000]


def run_1agent(prompt, timeout):
    command = [
        "1agent",
        "run",
        "--task",
        prompt,
        "--json",
        "--read-only",
        "--journal-dir",
        str(Path.home() / ".1a" / "blind-review-journal"),
        "--timeout-seconds",
        str(timeout),
    ]
    result = subprocess.run(command, capture_output=True, text=True, timeout=timeout + 30)
    if result.returncode != 0:
        raise RuntimeError(clean(result.stderr)[:2_000] or "1agent failed")
    payload = json.loads(result.stdout)
    answer = payload.get("answer", {}).get("conclusion", "")
    if not answer:
        raise RuntimeError("1agent returned no conclusion")
    return answer


def run_hermes(prompt, timeout, provider, model):
    result = subprocess.run(
        [
            "hermes",
            "--safe-mode",
            "--provider",
            provider,
            "--model",
            model,
            "-z",
            prompt,
        ],
        capture_output=True,
        text=True,
        timeout=timeout + 30,
    )
    if result.returncode != 0:
        raise RuntimeError(clean(result.stderr)[:2_000] or "hermes failed")
    answer = clean(result.stdout)
    if not answer:
        raise RuntimeError("hermes returned empty output")
    return answer


def load_golden(path):
    rows = [json.loads(line) for line in Path(path).read_text().splitlines() if line.strip()]
    if not rows:
        raise SystemExit("golden set is empty")
    return rows


def collect(args):
    golden = load_golden(args.golden)
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    limit = len(golden) if args.limit is None else min(args.limit, len(golden))
    if args.runner == "1agent":
        runner = run_1agent
    else:
        runner = lambda prompt, timeout: run_hermes(  # noqa: E731
            prompt, timeout, args.hermes_provider, args.hermes_model
        )
    written = 0
    with out.open("w") as handle:
        for row in golden[:limit]:
            try:
                answer = runner(row["prompt"], args.timeout)
                record = {
                    "id": row["id"],
                    "category": row.get("category"),
                    "prompt": row["prompt"],
                    "runner": args.runner,
                    "answer": answer,
                }
            except Exception as error:  # noqa: BLE001 - record and continue
                record = {
                    "id": row["id"],
                    "category": row.get("category"),
                    "prompt": row["prompt"],
                    "runner": args.runner,
                    "error": str(error)[:2_000],
                }
            handle.write(json.dumps(record, ensure_ascii=False) + "\n")
            handle.flush()
            written += 1
    print(f"wrote {written} answers to {out}")


def load_answers(path, runner):
    rows = {}
    for line in Path(path).read_text().splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        if row.get("runner") != runner:
            raise SystemExit(f"{path}: expected runner={runner}, got {row.get('runner')}")
        if "error" in row:
            print(f"skip {row['id']}: {row['error']}", file=sys.stderr)
            continue
        rows[row["id"]] = row
    return rows


def packet(args):
    golden = load_golden(args.golden)
    oneagent = load_answers(args.oneagent, "1agent")
    baseline = load_answers(args.baseline, "hermes")
    missing = [row["id"] for row in golden if row["id"] not in oneagent or row["id"] not in baseline]
    if missing and not args.allow_partial:
        raise SystemExit(f"missing answers for: {', '.join(missing)} (use --allow-partial)")
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    mapping_path = Path(args.mapping).expanduser()
    mapping_path.parent.mkdir(parents=True, exist_ok=True)
    mapping = {}
    prompts = []
    labels = []
    for row in golden:
        if row["id"] not in oneagent or row["id"] not in baseline:
            continue
        bit = hashlib.sha256(f"{args.seed}:{row['id']}".encode()).digest()[0] & 1
        if bit == 0:
            answer_a, answer_b = oneagent[row["id"]]["answer"], baseline[row["id"]]["answer"]
            mapping[row["id"]] = {"a": "1agent", "b": "hermes"}
        else:
            answer_a, answer_b = baseline[row["id"]]["answer"], oneagent[row["id"]]["answer"]
            mapping[row["id"]] = {"a": "hermes", "b": "1agent"}
        prompts.append(
            {
                "id": row["id"],
                "category": row.get("category"),
                "prompt": row["prompt"],
                "answer_a": answer_a,
                "answer_b": answer_b,
            }
        )
        for side in ("a", "b"):
            labels.append(
                {
                    "prompt_id": row["id"],
                    "side": side,
                    "first_answer_usable": None,
                    "pairwise_win": None,
                    "over_structured": None,
                    "unnecessary_followup": None,
                    "followup": None,
                    "followup_adopted": None,
                }
            )
    (out_dir / "prompts.jsonl").write_text(
        "\n".join(json.dumps(row, ensure_ascii=False) for row in prompts) + "\n"
    )
    (out_dir / "labels-template.jsonl").write_text(
        "\n".join(json.dumps(row, ensure_ascii=False) for row in labels) + "\n"
    )
    mapping_path.write_text(json.dumps(mapping, indent=2, ensure_ascii=False) + "\n")
    os.chmod(mapping_path, 0o600)
    print(f"packet: {len(prompts)} prompts, {len(labels)} label rows -> {out_dir}")
    print(f"sealed mapping -> {mapping_path} (keep private)")


def finalize(args):
    samples = []
    for line in Path(args.labels).read_text().splitlines():
        if not line.strip():
            continue
        row = json.loads(line)
        missing = [
            key
            for key in (
                "first_answer_usable",
                "over_structured",
                "unnecessary_followup",
                "followup",
                "followup_adopted",
            )
            if not isinstance(row.get(key), bool)
        ]
        if missing:
            raise SystemExit(f"{row.get('prompt_id')}/{row.get('side')}: missing {missing}")
        sample = {
            "first_answer_usable": row["first_answer_usable"],
            "over_structured": row["over_structured"],
            "unnecessary_followup": row["unnecessary_followup"],
            "followup": row["followup"],
            "followup_adopted": row["followup_adopted"],
        }
        if isinstance(row.get("pairwise_win"), bool):
            sample["pairwise_win"] = row["pairwise_win"]
        samples.append(sample)
    Path(args.out).write_text(
        "\n".join(json.dumps(sample, ensure_ascii=False) for sample in samples) + "\n"
    )
    print(f"wrote {len(samples)} answer-quality samples to {args.out}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    collect_parser = sub.add_parser("collect")
    collect_parser.add_argument("--runner", choices=["1agent", "hermes"], required=True)
    collect_parser.add_argument("--golden", default="evals/golden-set-v1.jsonl")
    collect_parser.add_argument("--out", required=True)
    collect_parser.add_argument("--limit", type=int)
    collect_parser.add_argument("--timeout", type=int, default=180)
    collect_parser.add_argument("--hermes-provider", default="deepseek")
    collect_parser.add_argument("--hermes-model", default="deepseek-flash")
    collect_parser.set_defaults(func=collect)

    packet_parser = sub.add_parser("packet")
    packet_parser.add_argument("--golden", default="evals/golden-set-v1.jsonl")
    packet_parser.add_argument("--oneagent", required=True)
    packet_parser.add_argument("--baseline", required=True)
    packet_parser.add_argument("--out-dir", required=True)
    packet_parser.add_argument("--mapping", required=True)
    packet_parser.add_argument("--seed", default="1agent-blind-review-v1")
    packet_parser.add_argument("--allow-partial", action="store_true")
    packet_parser.set_defaults(func=packet)

    finalize_parser = sub.add_parser("finalize")
    finalize_parser.add_argument("--labels", required=True)
    finalize_parser.add_argument("--out", required=True)
    finalize_parser.set_defaults(func=finalize)

    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
