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
if [[ "$status" != *"模型与运行限制"* || "$status" != *"刷新状态"* ]]; then
  echo "tui-interaction-check: /status did not render actionable state" >&2
  exit 1
fi

echo "tui-interaction-check: passed (greeting has no modal next menu; /help and /status render)"
