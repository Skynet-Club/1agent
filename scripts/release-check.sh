#!/usr/bin/env bash
# Release-gate wrapper for the 1Agent worktree.
# It does not replace the A-Gate/W-Gate checklists; it only proves the local
# software gates that can be automated from this repository.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export TMPDIR="${TMPDIR:-$HOME/.cache/1agent-tmp}"
mkdir -p "$TMPDIR"

# Keep the software tests hermetic: a developer's ~/.1a/config.json must not
# change provider routing or permission defaults. The TUI interaction check
# below explicitly unsets this to use the real provider.
export ONEAGENT_HOME="${ONEAGENT_HOME:-$TMPDIR/oneagent-home}"
mkdir -p "$ONEAGENT_HOME"

metrics_dir=""
run_soak=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --soak)
            run_soak=true
            shift
            ;;
        --metrics-dir)
            metrics_dir="${2:-}"
            if [[ -z "$metrics_dir" ]]; then
                echo "release-check: --metrics-dir needs a path" >&2
                exit 2
            fi
            shift 2
            ;;
        *)
            echo "release-check: unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

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
env -u ONEAGENT_HOME TMPDIR="$TMPDIR" ./scripts/tui-interaction-check.sh

if [[ -n "$metrics_dir" ]]; then
    echo "== metrics gate"
    ./scripts/gate-check.sh "$metrics_dir"
else
    echo "== metrics gate skipped (pass --metrics-dir DIR)"
fi

if $run_soak; then
    cycles="${SOAK_CYCLES:-5}"
    echo "== soak cycles=$cycles"
    for i in $(seq 1 "$cycles"); do
        echo "soak $i/$cycles"
        cargo test --offline --test a_gate_soak -- --ignored -q
    done
fi

echo "release-check: software gates passed"
echo "still required: A-Gate/W-Gate checklists, external audit, release package"
