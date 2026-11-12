#!/usr/bin/env bash
# Crash-injection recovery soak.
#
#   SOAK_RECOVERY_CYCLES=5 ./scripts/soak-recovery.sh
#   SOAK_RECOVERY_SECONDS=7200 ./scripts/soak-recovery.sh
#
# Each cycle runs a completed side-effect task, resumes from its checkpoint
# (the side effect must not repeat), and verifies that a mid-call or
# checkpoint-less journal fails closed. Duration mode paces with a short sleep
# so a two-hour run stays I/O-safe.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

export TMPDIR="${TMPDIR:-$HOME/.cache/1agent-tmp}"
mkdir -p "$TMPDIR"

profile="${SOAK_PROFILE:-release}"
if [[ "$profile" == "debug" ]]; then
    echo "recovery soak: profile=debug cycles=${SOAK_RECOVERY_CYCLES:-5} seconds=${SOAK_RECOVERY_SECONDS:-0}"
    cargo test --offline --test recovery_soak -- --ignored --nocapture
else
    echo "recovery soak: profile=release cycles=${SOAK_RECOVERY_CYCLES:-5} seconds=${SOAK_RECOVERY_SECONDS:-0}"
    cargo test --release --offline --test recovery_soak -- --ignored --nocapture
fi
