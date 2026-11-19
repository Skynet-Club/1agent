#!/bin/sh
# Reference subject for the synthetic memory family. It reads the seeded
# MEMORY.md and writes the expected answer; it is a fixture oracle, not an
# agent and not evidence of memory capability.
set -eu
task=""
workspace=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --task) task="$2"; shift 2 ;;
    --workspace) workspace="$2"; shift 2 ;;
    --journal-dir) shift 2 ;;
    *) shift ;;
  esac
done
: "${ONEASH_TASK_ID:?ONEASH_TASK_ID is required}"
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
script="$script_dir/references/$ONEASH_TASK_ID.sh"
[ -f "$script" ] || { echo "missing reference script $script" >&2; exit 2; }
cd "$workspace"
sh "$script"
printf '{"status":"completed","conclusion":"memory reference %s"}\n' "$ONEASH_TASK_ID"
