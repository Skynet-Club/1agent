#!/usr/bin/env python3
"""Verify an M1 1ASH report: schema plus the canonical hash-chain tip."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "schemas" / "1ash" / "report.schema.json"


def canonical_digest(value: dict) -> str:
    encoded = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(encoded.encode("utf-8")).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    report = json.loads(args.report.read_text(encoding="utf-8"))

    import jsonschema

    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    errors = list(
        jsonschema.Draft202012Validator(
            schema, format_checker=jsonschema.FormatChecker()
        ).iter_errors(report)
    )
    if errors:
        for error in errors:
            print(f"schema: {error.json_path}: {error.message}")
        return 1

    body = dict(report)
    tip = body.pop("hash_chain_tip", "")
    expected = "sha256:" + canonical_digest(body)
    if tip != expected:
        print(f"hash chain mismatch: recorded {tip}, expected {expected}")
        return 1
    if not report.get("signatures"):
        print(
            "warning: no signatures; the hash tip is an unkeyed self-checksum, "
            "not proof of origin"
        )
    print(
        "1ASH report v0 internally consistent: "
        f"{report['report_id']} · runs={len(report['runs'])} · {tip}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
