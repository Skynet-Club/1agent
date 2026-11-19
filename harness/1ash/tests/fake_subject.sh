#!/bin/sh
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
printf '%s\n' "$task" > "$workspace/task.txt"
printf 'fake artifact\n' > "$workspace/out.txt"
printf '{"status":"completed","conclusion":"fake subject wrote out.txt"}\n'
