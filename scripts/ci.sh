#!/usr/bin/env bash
# Local CI. No network, no remote, no CI service.
#
# Runs the gates, rebuilds the release binary, and appends one evidence line to
# a history file outside the repository. Logs rotate after 30 days.
#
#   ./scripts/ci.sh
#
# Exit status equals the gate status, so cron mail / a caller can see failures.
set -uo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/1agent-ci"
log_dir="$state_dir/logs"
mkdir -p "$log_dir"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
log="$log_dir/$stamp.log"
commit="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

dirty="clean"
if [[ -n "$(git status --porcelain 2>/dev/null)" ]]; then
  dirty="dirty"
fi

(
  set -e
  printf 'commit %s (%s)\n' "$commit" "$dirty"
  printf 'started %s\n\n' "$stamp"
  ./scripts/check-attribution.sh
  ./scripts/verify.sh
  ./scripts/verify-claims.sh --run
  cargo build --offline --release
) >"$log" 2>&1
status=$?

digest="none"
if [[ -x target/release/first-principles-agent ]]; then
  digest="$(sha256sum target/release/first-principles-agent | cut -d' ' -f1)"
fi

printf '%s\t%s\t%s\t%s\t%s\n' \
  "$stamp" "$status" "$commit" "$dirty" "$digest" >>"$state_dir/history.tsv"
ln -sfn "$log" "$state_dir/latest.log"
find "$log_dir" -name '*.log' -mtime +30 -delete 2>/dev/null || true

printf 'ci run %s  status=%s  commit=%s (%s)\n' "$stamp" "$status" "$commit" "$dirty"
printf 'binary sha256 %s\n' "$digest"
printf 'log     %s\n' "$log"
printf 'history %s\n' "$state_dir/history.tsv"

exit "$status"
