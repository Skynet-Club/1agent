#!/usr/bin/env python3
"""PTY acceptance for the end-of-turn next-action menu.

Drives the real TUI in a pseudo terminal with the deterministic provider, sends
one task, and asserts that the menu panel appears with a default entry. Offline,
no credentials, no model calls.

    python3 scripts/pty_next_actions.py

Exits 0 only if the panel is observed. Text matching is done on the ANSI-stripped
stream, so it tolerates repaint noise.
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
JOURNAL = os.path.join(ROOT, ".journal-pty")
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b[()][A-Z0-9]")


def drive() -> str:
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(
            BINARY,
            [BINARY, "tui", "--provider", "mock", "--journal-dir", JOURNAL],
        )
    # A pty starts with a 0x0 window; the TUI renders nothing until it has a size.
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

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
    os.write(fd, "你好\r".encode())
    drain(6.0)
    os.write(fd, b"\x11")
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
    checks = {
        "welcome rendered": "1-Agent" in screen or "会话" in screen,
        "next-action panel": "下一步" in screen,
        "default entry marked": "（默认）" in screen,
    }
    for label, passed in checks.items():
        print(f"  {'ok  ' if passed else 'FAIL'}  {label}")
    if all(checks.values()):
        print("pty acceptance passed")
        return 0
    print("pty acceptance failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
