#!/usr/bin/env python3
"""Validate the 1ASH v1 schema examples with the optional jsonschema module.

Usage: python3 scripts/check-1ash-schemas.py
Exit 2 if jsonschema is not installed; this is a design gate, not a runtime
dependency.
"""
import json
import pathlib
import sys

try:
    import jsonschema
except ImportError:
    print("jsonschema is not installed; install it to run the full 1ASH schema gate")
    raise SystemExit(2)

root = pathlib.Path(__file__).resolve().parent.parent / "schemas" / "1ash"


def load(name):
    return json.loads((root / name).read_text())


schema_paths = {
    "task": root / "task.schema.json",
    "run": root / "run.schema.json",
    "trace": root / "trace.schema.json",
    "report": root / "report.schema.json",
}
schemas = {name: json.loads(path.read_text()) for name, path in schema_paths.items()}
examples = {
    "task": load("examples/task.example.json"),
    "run": load("examples/run.example.json"),
    "report": load("examples/report.example.json"),
}
for name, example in examples.items():
    jsonschema.Draft202012Validator(schemas[name]).validate(example)
    print(f"{name}: ok")

trace_schema = schemas["trace"]
for line_number, line in enumerate((root / "examples/trace.example.jsonl").read_text().splitlines(), 1):
    if not line.strip():
        continue
    jsonschema.Draft202012Validator(trace_schema).validate(json.loads(line))
    print(f"trace line {line_number}: ok")

print("1ASH schema examples passed")
