#!/usr/bin/env bash
# PTY-level interaction checks for the standalone TUI.
#
# Requires pyte in PYTE_PYTHON (defaults to the local xlsx venv). The screens
# are rendered by a terminal emulator, not scraped from raw ANSI.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
PYTE_PYTHON="${PYTE_PYTHON:-$HOME/.cache/xlsxenv/bin/python}"
if [[ ! -x "$PYTE_PYTHON" ]]; then
  echo "tui-interaction-check: skipped (pyte interpreter not found: $PYTE_PYTHON)" >&2
  exit 0
fi

render() {
  timeout 30 "$PYTE_PYTHON" scripts/pty_screenshot.py --cols 120 --rows 40 "$@" 2>/dev/null \
    | tr -d '[:space:]'
}

greeting="$(render --keys $'你好，模型连接测试\r' --settle 4)"
if [[ "$greeting" != *"已完成"* ]]; then
  echo "tui-interaction-check: greeting did not finish" >&2
  exit 1
fi
if [[ "$greeting" == *"下一步"* || "$greeting" == *"/frame（默认）"* ]]; then
  echo "tui-interaction-check: standalone greeting opened the next-action menu" >&2
  exit 1
fi

help="$(render --keys $'/help\r' --settle 2)"
if [[ "$help" != *"命令"* || "$help" != *"/frame"* ]]; then
  echo "tui-interaction-check: /help did not render the command panel" >&2
  exit 1
fi

status="$(render --keys $'/status\r' --settle 2)"
if [[ "$status" != *"模型与运行限制"* || "$status" != *"剩余空间"* ]]; then
  echo "tui-interaction-check: /status did not render inline state" >&2
  exit 1
fi

mode="$(render --keys $'/mode\r' --settle 2)"
if [[ "$mode" != *"切换模式"* || "$mode" != *"证据研究"* ]]; then
  echo "tui-interaction-check: /mode did not render the mode menu" >&2
  exit 1
fi

mode_key="$(render --keys $'\x14' --settle 2)"
if [[ "$mode_key" != *"切换模式"* ]]; then
  echo "tui-interaction-check: Ctrl+T did not open the portable mode menu" >&2
  exit 1
fi

drawer="$(render --keys $'/' --settle 2)"
if [[ "$drawer" != *"/help"* || "$drawer" != *"命令"* || "$drawer" != *"会话"* ]]; then
  echo "tui-interaction-check: slash drawer did not render categories" >&2
  exit 1
fi

completion="$(render --keys $'/fra\t' --settle 2)"
if [[ "$completion" != *"/frame"* || "$completion" != *"参数"* ]]; then
  echo "tui-interaction-check: Tab completion did not add the argument hint" >&2
  exit 1
fi

confirmation="$(render --keys $'/clear\r' --settle 2)"
if [[ "$confirmation" != *"危险操作"* || "$confirmation" != *"/clear"* ]]; then
  echo "tui-interaction-check: destructive command did not ask for confirmation" >&2
  exit 1
fi

panel_focus="$(render --keys $'/help\rabc' --settle 2)"
if [[ "$panel_focus" != *"命令"* || "$panel_focus" == *"abc"* ]]; then
  echo "tui-interaction-check: command panel did not isolate the composer" >&2
  exit 1
fi

"$PYTE_PYTHON" scripts/pty_backspace.py
python3 scripts/pty_edit_stability.py

echo "tui-interaction-check: passed (greeting has no modal next menu; /help, /status, /mode, Ctrl+T, categorized drawer, Tab completion, destructive confirmation, panel focus, Backspace/Ctrl+Q and mixed paste/space/backspace editing)"
