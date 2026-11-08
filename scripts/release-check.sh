#!/usr/bin/env bash
# Release-gate wrapper for the 1Agent worktree.
# It does not replace the A-Gate/W-Gate checklists; it only proves the local
# software gates that can be automated from this repository.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export TMPDIR="${TMPDIR:-$HOME/.cache/1agent-tmp}"
mkdir -p "$TMPDIR"

echo "== fmt"
cargo fmt --all --check

echo "== clippy"
cargo clippy --offline --all-targets -- -D warnings

echo "== tests"
cargo test --offline

echo "== http e2e"
cargo test --offline --test http_surface_e2e -- --ignored

echo "== ci"
./scripts/ci.sh

echo "== tui interaction"
./scripts/tui-interaction-check.sh

if [[ "${1:-}" == "--soak" ]]; then
    cycles="${SOAK_CYCLES:-5}"
    echo "== soak cycles=$cycles"
    for i in $(seq 1 "$cycles"); do
        echo "soak $i/$cycles"
        cargo test --offline --test a_gate_soak -- --ignored -q
    done
fi

echo "release-check: software gates passed"
echo "still required: A-Gate/W-Gate checklists, external audit, release package"
