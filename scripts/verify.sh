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

# Platform branching is permitted only in presentation adapters (docs/adr/0006);
# it is forbidden in domain and application modules (docs/architecture.md).
if grep -rnF 'cfg(target_os' src --exclude=tui_terminal.rs >/dev/null; then
  echo "platform-specific branching leaked into src/"
  exit 1
fi

./scripts/verify-claims.sh

echo "verification passed"
