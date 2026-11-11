#!/usr/bin/env bash
# Reject AI-tool attribution and the product name "Claude Code".
#
# This project is released under the project's own identity. Tool bylines are
# not part of the product and must not re-enter the commit history or the tree.
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

status=0
pattern='co-authored-by:.*claude|generated with .*claude|noreply@anthropic\.com|claude code|claude-code|claude_code'

if git log HEAD --format='%B' | grep -Eiq "$pattern"; then
  echo "attribution: commit history contains a forbidden Claude marker" >&2
  status=1
fi

if git ls-files -z --cached --others --exclude-standard \
  | grep -zv '^scripts/check-attribution.sh$' \
  | xargs -0 -r grep -InE -i "$pattern" >&2; then
  echo "attribution: working tree contains a forbidden Claude marker" >&2
  status=1
fi

if [[ "$status" -eq 0 ]]; then
  echo "attribution: clean"
fi
exit "$status"
