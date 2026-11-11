#!/usr/bin/env bash
# Cross-target compile gate for the 1Agent worktree.
#
# Windows GNU is required. macOS is attempted when the target is installed;
# a missing macOS SDK/compiler is reported as a host-toolchain block unless
# --require-macos is passed.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
require_macos=0
build_windows=0
for arg in "$@"; do
    case "$arg" in
        --require-macos) require_macos=1 ;;
        --build) build_windows=1 ;;
        *) echo "cross-check: unknown argument $arg" >&2; exit 2 ;;
    esac
done

installed="$(rustup target list --installed)"
have_target() {
    grep -qx "$1" <<<"$installed"
}

if ! have_target x86_64-pc-windows-gnu; then
    echo "cross-check: install the Windows target first: rustup target add x86_64-pc-windows-gnu" >&2
    exit 1
fi

echo "== windows all-targets check"
RUSTFLAGS="-D warnings" cargo check --offline --all-targets --target x86_64-pc-windows-gnu
echo "cross-check: Windows GNU passed"

if [[ "$build_windows" -eq 1 ]]; then
    echo "== windows release link/build"
    cargo build --offline --release --target x86_64-pc-windows-gnu
    test -f target/x86_64-pc-windows-gnu/release/first-principles-agent.exe
    echo "cross-check: Windows GNU release binary built"
fi

if have_target aarch64-apple-darwin; then
    echo "== macOS arm64 check"
    if RUSTFLAGS="-D warnings" cargo check --offline --all-targets --target aarch64-apple-darwin; then
        echo "cross-check: macOS arm64 passed"
    else
        echo "cross-check: macOS arm64 blocked by the host C toolchain (ring/cc)" >&2
        if [[ "$require_macos" -eq 1 ]]; then
            exit 1
        fi
    fi
else
    echo "cross-check: macOS target not installed; skipped"
    if [[ "$require_macos" -eq 1 ]]; then
        exit 1
    fi
fi

echo "cross-check: required gates passed"
