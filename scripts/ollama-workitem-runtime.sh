#!/bin/sh
# Workshop runtime adapter for local Ollama multi-agent tests.
# `skynet-workshop-agent-run` appends the task text as the last argument and
# runs us with cwd set to the Work Item workspace.
set -eu

task="${1:?task text required}"
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec "$root/target/debug/first-principles-agent" run \
  --config "$root/configs/ollama-local.json" \
  --tool-query write_file \
  --task "$task" \
  --json \
  --read-root "$PWD" \
  --max-steps 6 \
  --max-tool-calls 4 \
  --max-provider-attempts 2 \
  --timeout-seconds 300 \
  --no-network
