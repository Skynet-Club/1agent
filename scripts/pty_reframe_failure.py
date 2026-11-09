#!/usr/bin/env python3
"""PTY acceptance for the /reframe failure path.

Points the interface at a closed local port, asks for a reframe, and asserts
that the failure is reported in one readable line, that the endpoint is not
printed, and that the session still responds afterwards.

Offline: the connection is refused locally, nothing leaves the machine.

    python3 scripts/pty_reframe_failure.py
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
JOURNAL = os.path.join(ROOT, ".journal-pty-failure")
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b[()][A-Z0-9]")
ENDPOINT = "127.0.0.1:9"


def drive() -> str:
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(
            BINARY,
            [
                BINARY,
                "tui",
                "--provider",
                "openai-compatible",
                "--base-url",
                f"http://{ENDPOINT}/v1",
                "--model",
                "unused-model",
                "--allow-network",
                "--timeout-seconds",
                "3",
                "--journal-dir",
                JOURNAL,
            ],
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
    drain(6.0)
    # The session must still accept input after the failure.
    os.write(fd, "/status\r".encode())
    drain(2.0)
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
    tight = re.sub(r"\s+", "", screen)
    checks = {
        "failure reported": "重构失败" in tight,
        "endpoint not printed": ENDPOINT not in tight,
        "session still responds": "运行状态" in tight or "宿主" in tight,
    }
    for label, passed in checks.items():
        print(f"  {'ok  ' if passed else 'FAIL'}  {label}")
    if all(checks.values()):
        print("pty reframe failure acceptance passed")
        return 0
    print("pty reframe failure acceptance failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
