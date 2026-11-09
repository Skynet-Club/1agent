#!/usr/bin/env bash
# Reproduce the raw-material counts cited in docs/thesis/00-problem.md §7.
# Read-only. No network. Safe to run at any time.
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

journal_glob=(.journal/*.jsonl)
if [[ ! -e "${journal_glob[0]}" ]]; then
  printf 'no journal files under %s/.journal\n' "$project_dir" >&2
  exit 1
fi

runs=$(jq -r 'select(.event.type=="run_started") | .run_id' "${journal_glob[@]}" 2>/dev/null | sort -u | wc -l | tr -d ' ')
objectives=$(jq -r 'select(.event.type=="run_started") | .event.data.request.objective' "${journal_glob[@]}" 2>/dev/null | wc -l | tr -d ' ')

printf 'journal files        : %s\n' "$(ls "${journal_glob[@]}" | wc -l | tr -d ' ')"
printf 'distinct runs        : %s\n' "$runs"
printf 'run_started events   : %s\n' "$objectives"

printf '\nmode distribution:\n'
jq -r 'select(.event.type=="run_started") | .event.data.mode' "${journal_glob[@]}" 2>/dev/null \
  | sort | uniq -c | sort -rn

printf '\nstop reasons:\n'
jq -r 'select(.event.type=="run_stopped") | .event.data.reason' "${journal_glob[@]}" 2>/dev/null \
  | sort | uniq -c | sort -rn

printf '\nobjectives longer than 12 chars (deduplicated, top 15):\n'
jq -r 'select(.event.type=="run_started") | .event.data.request.objective' "${journal_glob[@]}" 2>/dev/null \
  | awk 'length($0) > 12' | sort | uniq -c | sort -rn | head -15

printf '\ndocs mentioning requirement/intent regression:\n'
if grep -l '用户需求\|意图\|需求分析\|需求挖掘\|需求回溯' docs/status-v0.*.md docs/audit-v0.*.md 2>/dev/null; then
  true
else
  printf '  0 files\n'
fi
