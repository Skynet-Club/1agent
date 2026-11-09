#!/usr/bin/env bash
# Import a filled label CSV into the annotation store used by `1agent annotate`.
#
#   ./scripts/reframing/import_labels.sh labels-a.csv alice
#
# The CSV has three columns: case_id,label,note
# label must be supported | unsupported | needs_evidence
set -euo pipefail

csv="${1:?usage: import_labels.sh <csv> <annotator> [store]}"
annotator="${2:?usage: import_labels.sh <csv> <annotator> [store]}"
store="${3:-.annotations}"

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$project_dir"

binary="target/release/first-principles-agent"
[[ -x "$binary" ]] || { printf 'missing %s; run cargo build --release\n' "$binary" >&2; exit 1; }

imported=0
skipped=0
while IFS=, read -r case_id label note; do
  [[ "$case_id" == "case_id" ]] && continue
  [[ -z "$case_id" ]] && continue
  if [[ -z "$label" ]]; then
    skipped=$((skipped + 1))
    continue
  fi
  case "$label" in
    supported|unsupported|needs_evidence) ;;
    *) printf 'bad label "%s" for %s\n' "$label" "$case_id" >&2; exit 1 ;;
  esac
  "$binary" annotate add \
    --store "$store" \
    --case-id "$case_id" \
    --annotator "$annotator" \
    --label "$label" \
    --note "$note" >/dev/null
  imported=$((imported + 1))
done < "$csv"

printf 'imported %d annotation(s) as %s into %s' "$imported" "$annotator" "$store"
[[ $skipped -gt 0 ]] && printf ' (%d blank row(s) skipped)' "$skipped"
printf '\n'
