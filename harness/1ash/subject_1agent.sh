#!/bin/sh
# M1 subject adapter: run 1agent inside the task workspace and emit its JSON.
set -eu
task=""
workspace=""
journal=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --task) task="$2"; shift 2 ;;
    --workspace) workspace="$2"; shift 2 ;;
    --journal-dir) journal="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if [ -z "$task" ] || [ -z "$workspace" ] || [ -z "$journal" ]; then
  echo "usage: $0 --task TEXT --workspace DIR --journal-dir DIR" >&2
  exit 2
fi
cd "$workspace"
config="${ONEASH_1AGENT_CONFIG:-$HOME/.1a/config.json}"
max_steps="${ONEASH_1AGENT_MAX_STEPS:-12}"
max_tools="${ONEASH_1AGENT_MAX_TOOL_CALLS:-12}"
if [ ! -f "$config" ]; then
  echo "1ash: no 1Agent config at $config; set ONEASH_1AGENT_CONFIG" >&2
  exit 2
fi

# `1agent run` auto-loads the default config, so CLI provider flags cannot
# override it, and config manifests cap risk below the Execute mode this
# harness needs. Copy the selected config into the run's journal directory and
# raise only the two fields the harness requires; credentials stay referenced
# by path, never copied into the temporary file.
cfg="$journal/1ash-run-config.json"
python3 - "$config" "$cfg" <<'PY'
import json, os, sys

source, target = sys.argv[1], sys.argv[2]
config = json.load(open(source, encoding="utf-8"))
permissions = config.setdefault("permissions", {})
permissions["allow_network"] = True
providers = config.get("providers") or []
if not providers:
    raise SystemExit("1ash: config has no providers")
for provider in providers:
    if "execute" in (provider.get("supported_modes") or []):
        provider["max_risk"] = "high"
fd = os.open(target, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(fd, "w", encoding="utf-8") as handle:
    json.dump(config, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
PY
trap 'rm -f "$cfg"' EXIT

set -- run --config "$cfg" --task "$task" \
  --read-root "$workspace" --journal-dir "$journal" \
  --max-steps "$max_steps" --max-tool-calls "$max_tools" --json
"${ONEASH_1AGENT_BIN:-1agent}" "$@"
