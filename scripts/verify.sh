#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_dir"

cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
cargo test --offline
cargo run --offline --quiet -- eval --suite evals/core.json

if [[ "${AGENT_RUN_LOOPBACK_TESTS:-0}" == "1" ]]; then
  cargo test --offline --test openai_adapter -- --ignored
fi

if rg -n 'cfg\(target_os' src >/dev/null; then
  echo "platform-specific branching leaked into src/"
  exit 1
fi

echo "verification passed"
