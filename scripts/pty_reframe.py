#!/usr/bin/env python3
"""PTY acceptance for /reframe in the TUI.

Runs the real interface with the offline provider, asks for a reframe, then
decides, and asserts both that the proposal panel appeared and that the decision
was recorded. Offline, no credentials, repeatable.

    python3 scripts/pty_reframe.py
"""

import fcntl
import os
import pty
import re
import select
import struct
import sys
import termios
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BINARY = os.path.join(ROOT, "target/release/first-principles-agent")
JOURNAL = os.path.join(ROOT, ".journal-pty-reframe")
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b[()][A-Z0-9]")


def drive() -> str:
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(
            BINARY,
            [BINARY, "tui", "--provider", "mock", "--journal-dir", JOURNAL],
        )
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 44, 120, 0, 0))

    chunks = []

    def drain(seconds):
        end = time.time() + seconds
        while time.time() < end:
            ready, _, _ = select.select([fd], [], [], 0.2)
            if not ready:
                continue
            try:
                data = os.read(fd, 65536)
            except OSError:
                return
            if not data:
                return
            chunks.append(data)

    drain(3.0)
    os.write(fd, "/reframe 我要买个电钻\r".encode())
    drain(3.0)
    # Enter selects the highlighted menu entry, which is how a person accepts;
    # typing "/reframe accept 1" is the scripting path and is covered by tests.
    os.write(fd, b"\r")
    drain(3.0)
    os.write(fd, b"\x11\r")
    drain(2.0)
    try:
        os.waitpid(pid, 0)
    except ChildProcessError:
        pass
    return ANSI.sub("", b"".join(chunks).decode("utf-8", "replace"))


def main():
    if not os.access(BINARY, os.X_OK):
        print("build the release binary first", file=sys.stderr)
        return 1

    screen = drive()
    # The renderer collapses spacing inside panels, so compare with whitespace
    # removed rather than against the literal strings that were submitted.
    tight = re.sub(r"\s+", "", screen)
    checks = {
        "proposal shown": "判断手段" in tight,
        "question shown": "你主要打算用它做什么？" in tight,
        "options offered": "采纳「家里偶尔用」" in tight,
        "decision recorded by Enter": "采纳第1项" in tight,
        "journal written": ".journal-pty-reframe/" in tight,
    }
    for label, passed in checks.items():
        print(f"  {'ok  ' if passed else 'FAIL'}  {label}")
    if all(checks.values()):
        print("pty reframe acceptance passed")
        return 0
    print("pty reframe acceptance failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
