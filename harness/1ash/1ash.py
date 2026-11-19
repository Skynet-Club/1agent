#!/usr/bin/env python3
"""Small 1ASH harness CLI (M1/M2/M3/M4 local commands)."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
VENV = Path.home() / ".cache" / "1ash-venv" / "bin" / "python3"


def python() -> str:
    return str(VENV) if VENV.exists() else sys.executable


def run(script: Path, args: list[str]) -> int:
    return subprocess.run([python(), str(script), *args], check=False).returncode


def main() -> int:
    parser = argparse.ArgumentParser(prog="1ash")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("m1", help="run fake/no-op M1 adapter checks")
    sub.add_parser("suite", help="run the synthetic 30-task suite")
    sub.add_parser("safety", help="run the synthetic safety calibration")
    verify = sub.add_parser("verify", help="verify an 1ash.report/v1 report")
    verify.add_argument("report", type=Path)
    pilot = sub.add_parser("pilot", help="build the local pilot tarball")
    pilot.add_argument("version", nargs="?", default="0.1.0")
    sub.add_parser("docker", help="run the Docker-isolated fake/no-op checks")
    args = parser.parse_args()

    if args.command == "m1":
        return run(ROOT / "tests" / "run_m1.py", [])
    if args.command == "suite":
        return run(ROOT / "suite" / "run_suite.py", [])
    if args.command == "safety":
        return run(ROOT / "suite" / "run_safety.py", [])
    if args.command == "verify":
        return run(ROOT / "verify_report.py", [str(args.report)])
    if args.command == "pilot":
        return subprocess.run(
            [str(ROOT / "make_pilot.sh"), args.version], check=False
        ).returncode
    if args.command == "docker":
        return run(ROOT / "tests" / "run_m1_docker.py", [])
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
