#!/usr/bin/env bash
# Local hard-gate check over a journal directory.
#
#   ./scripts/gate-check.sh <journal-dir>
#
# Thresholds are basis points (10000 = 100%). A missing sample fails closed.
# M6 (zero security incidents) is not derivable from journals; run the
# security suite separately.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

journal_dir="${1:-${ONEAGENT_GATE_JOURNAL_DIR:-}}"
if [[ -z "$journal_dir" || ! -d "$journal_dir" ]]; then
    echo "gate-check: journal directory required: ./scripts/gate-check.sh DIR" >&2
    exit 2
fi

args=(
    metrics
    --journal-dir "$journal_dir"
    --require-recovery-bps "${GATE_MIN_RECOVERY_BPS:-9500}"
    --require-replay-bps "${GATE_MIN_REPLAY_BPS:-10000}"
    --require-audit-complete-bps "${GATE_MIN_AUDIT_BPS:-10000}"
    --require-compacted-replay-bps "${GATE_MIN_COMPACTED_BPS:-10000}"
)

if [[ -n "${GATE_MIN_TOOL_SUCCESS_BPS:-}" ]]; then
    args+=(--require-tool-success-bps "$GATE_MIN_TOOL_SUCCESS_BPS")
fi
if [[ -n "${GATE_MIN_CRASH_FREE_BPS:-}" ]]; then
    args+=(--require-crash-free-bps "$GATE_MIN_CRASH_FREE_BPS")
fi
if [[ -n "${GATE_MAX_DENIED_OBSERVATIONS:-}" ]]; then
    args+=(--max-denied-observations "$GATE_MAX_DENIED_OBSERVATIONS")
fi

echo "gate-check: journal_dir=$journal_dir"
cargo run --offline --quiet -- "${args[@]}"
echo "gate-check: local hard gates passed"
echo "gate-check: M6 security incidents still require the separate security suite"
