#!/usr/bin/env python3
"""Render the TUI to text through a real pseudo terminal.

Strips nothing by hand: a terminal emulator applies the cursor movement, so the
output is the screen a person would see. Useful for checking alignment and
spacing without a human at the keyboard.

    ~/.cache/xlsxenv/bin/python scripts/pty_screenshot.py [--cols 120] [--rows 40] [--keys "/help\r"]

Requires pyte (installed in the dedicated venv, not in the system interpreter).
"""

import argparse
import fcntl
import os
import pty
import select
import shutil
import struct
import sys
import tempfile
import termios
import time

import pyte
from wcwidth import wcwidth

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BINARY = os.path.join(ROOT, "target/release/first-principles-agent")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cols", type=int, default=120)
    parser.add_argument("--rows", type=int, default=40)
    parser.add_argument("--keys", default="", help="input to send after startup, \\r for Enter")
    parser.add_argument("--settle", type=float, default=2.5)
    args = parser.parse_args()

    screen = pyte.Screen(args.cols, args.rows)
    stream = pyte.ByteStream(screen)
    journal_dir = tempfile.mkdtemp(prefix="1agent-pty-")

    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(
            BINARY,
            [BINARY, "tui", "--provider", "mock", "--journal-dir", journal_dir],
        )
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", args.rows, args.cols, 0, 0))

    def drain(seconds):
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([fd], [], [], 0.15)
            if not ready:
                continue
            try:
                data = os.read(fd, 65536)
            except OSError:
                return
            if not data:
                return
            stream.feed(data)

    drain(args.settle)
    if args.keys:
        os.write(fd, args.keys.encode().replace(b"\\r", b"\r"))
        drain(args.settle)
    # Snapshot before quitting: leaving the alternate screen restores the blank
    # primary buffer, so a post-exit capture shows nothing. Skip the empty stub
    # cell pyte stores after each wide character, and treat an empty cell as a
    # space (`Screen.display` trips over empty data in this pyte version).
    rows = []
    for y in range(screen.lines):
        line = screen.buffer[y]
        row = []
        skip_stub = False
        for x in range(screen.columns):
            if skip_stub:
                skip_stub = False
                continue
            data = line[x].data
            if not data:
                row.append(" ")
                continue
            row.append(data)
            skip_stub = wcwidth(data[0]) == 2
        rows.append("".join(row).rstrip())

    os.write(fd, b"\x11\r")
    drain(1.0)
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass
    shutil.rmtree(journal_dir, ignore_errors=True)

    for line in rows:
        print(line)
    return 0


if __name__ == "__main__":
    sys.exit(main())
