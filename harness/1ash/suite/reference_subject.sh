#!/bin/sh
# Reference subject for the synthetic M2 suite. It executes the task's
# reference script selected by ONEASH_TASK_ID; it is a solver oracle, not an
# agent and not evidence of model capability.
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
: "${ONEASH_REFERENCE_DIR:?ONEASH_REFERENCE_DIR is required}"
script="$ONEASH_REFERENCE_DIR/$ONEASH_TASK_ID.sh"
[ -f "$script" ] || { echo "missing reference script $script" >&2; exit 2; }
cd "$workspace"
sh "$script"
printf '{"status":"completed","conclusion":"reference solution %s"}\n' "$ONEASH_TASK_ID"
