#!/usr/bin/env bash
# Run a style fixture through the CLI, one journal directory per fixture.
#
#   ./scripts/answer_style/run_fixture.sh fixtures/answer_style/style-v1.json .journal-style-fresh
#
# Provider settings come from the environment: AGENT_API_KEY, AGENT_MODEL,
# AGENT_BASE_URL. This is a live call and it costs money.
set -euo pipefail

fixture="${1:?usage: run_fixture.sh <fixture.json> <journal-dir>}"
journal_dir="${2:?usage: run_fixture.sh <fixture.json> <journal-dir>}"

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$project_dir"

binary="target/release/first-principles-agent"
[[ -x "$binary" ]] || { printf 'build the release binary first\n' >&2; exit 1; }

mkdir -p "$journal_dir"
ok=0
fail=0
while IFS=$'\t' read -r id objective; do
  if "$binary" run \
    --provider openai-compatible \
    --model "${AGENT_MODEL:-deepseek-flash}" \
    --base-url "${AGENT_BASE_URL:-https://api.deepseek.com/v1}" \
    --allow-network \
    --journal-dir "$journal_dir" \
    --task "$objective" >/dev/null 2>&1; then
    ok=$((ok + 1))
  else
    fail=$((fail + 1))
    printf 'failed: %s\n' "$id" >&2
  fi
done < <(jq -r '.tasks[] | [.id, .objective] | @tsv' "$fixture")

printf 'runs ok=%d fail=%d -> %s\n' "$ok" "$fail" "$journal_dir"
[[ $fail -eq 0 ]]
