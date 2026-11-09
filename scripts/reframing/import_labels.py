#!/usr/bin/env python3
"""Import filled labels from a readable sheet into the annotation store.

Accepts the CSV produced by make_label_sheets.py (looked up by header name, so
extra columns and reordering are fine) or the Markdown variant where the label
cell has been filled in.

    python3 scripts/reframing/import_labels.py --sheet labels-A.csv --annotator alice
    python3 scripts/reframing/import_labels.py --sheet 01-标注表.md --annotator bob

Rows whose label is still `?` or empty are skipped and counted.
"""

import argparse
import csv
import re
import subprocess
import sys
from pathlib import Path

VALID = {"supported", "unsupported", "needs_evidence"}
ROOT = Path(__file__).resolve().parents[2]
BINARY = ROOT / "target/release/first-principles-agent"


def from_csv(path):
    text = path.read_text(encoding="utf-8-sig")
    reader = csv.DictReader(text.splitlines())
    missing = {"case_id", "label"} - set(reader.fieldnames or [])
    if missing:
        raise SystemExit(f"{path}: missing column(s) {sorted(missing)}")
    for row in reader:
        yield (row.get("case_id", "") or "").strip(), (row.get("label", "") or "").strip(), (
            row.get("note", "") or ""
        ).strip()


def from_markdown(path):
    pattern = re.compile(r"^\|\s*(\d+)\s*\|.*\|\s*([^|]*)\s*\|\s*([^|]*)\s*\|\s*$")
    in_table = False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("| 编号 |"):
            in_table = True
            continue
        if not in_table:
            continue
        if line.startswith("|---"):
            continue
        if not line.startswith("|"):
            break
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) < 7:
            continue
        label = cells[4].strip("` ").strip()
        note = cells[5].strip()
        case_id = cells[6].strip("` ").strip()
        yield case_id, label, note


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--sheet", required=True)
    parser.add_argument("--annotator", required=True)
    parser.add_argument("--store", default=".annotations")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    path = Path(args.sheet).expanduser()
    if not path.is_file():
        raise SystemExit(f"no such sheet: {path}")

    entries = from_csv(path) if path.suffix.lower() == ".csv" else from_markdown(path)
    imported = 0
    skipped = 0
    bad = 0
    for case_id, label, note in entries:
        label = label.strip().strip("`").lower()
        if not case_id and label not in VALID:
            skipped += 1
            continue
        if label not in VALID:
            skipped += 1
            continue
        if args.dry_run:
            imported += 1
            continue
        result = subprocess.run(
            [
                str(BINARY),
                "annotate",
                "add",
                "--store",
                args.store,
                "--case-id",
                case_id,
                "--annotator",
                args.annotator,
                "--label",
                label,
                "--note",
                note,
            ],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            bad += 1
            print(f"failed {case_id}: {result.stderr.strip()}", file=sys.stderr)
        else:
            imported += 1

    print(f"imported {imported}, skipped {skipped} (blank or '?'), failed {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
