#!/usr/bin/env python3
"""PTY acceptance for cancelling a running /reframe.

Starts a local socket that accepts a connection and never answers, points the
interface at it, asks for a reframe, then sends Ctrl+C after a moment. Asserts
that the cancellation is reported and the session keeps working.

Offline: the socket is on loopback and never sends anything.

    python3 scripts/pty_reframe_cancel.py
"""

import fcntl
import os
import pty
import re
import select
import socket
import struct
import sys
import termios
import threading
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BINARY = os.path.join(ROOT, "target/release/first-principles-agent")
JOURNAL = os.path.join(ROOT, ".journal-pty-cancel")
ANSI = re.compile(r"\x1b\[[0-9;?]*[a-zA-Z]|\x1b[()][A-Z0-9]")


class BlackHole:
    """Accepts connections and never replies, so the client keeps waiting."""

    def __init__(self):
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind(("127.0.0.1", 0))
        self.sock.listen(4)
        self.port = self.sock.getsockname()[1]
        self.stop = False
        self.held = []
        threading.Thread(target=self._serve, daemon=True).start()

    def _serve(self):
        while not self.stop:
            ready, _, _ = select.select([self.sock], [], [], 0.2)
            if not ready:
                continue
            try:
                connection, _ = self.sock.accept()
            except OSError:
                return
            self.held.append(connection)

    def close(self):
        self.stop = True
        for connection in self.held:
            try:
                connection.close()
            except OSError:
                pass
        try:
            self.sock.close()
        except OSError:
            pass


def drive(port: int) -> str:
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
                f"http://127.0.0.1:{port}/v1",
                "--model",
                "hanging-model",
                "--allow-network",
                "--timeout-seconds",
                "30",
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
    # Give the request time to actually start; cancelling before it begins only
    # tests the drawer, not the running call.
    drain(3.5)
    os.write(fd, b"\x03")  # Ctrl+C
    drain(2.5)
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

    hole = BlackHole()
    try:
        screen = drive(hole.port)
    finally:
        hole.close()

    tight = re.sub(r"\s+", "", screen)
    checks = {
        "cancel is offered": "Ctrl+C取消" in tight,
        "cancellation reported": "已取消这次判断" in tight,
        "session still responds": "运行状态" in tight or "宿主" in tight,
        "no endpoint printed": "127.0.0.1" not in tight,
    }
    for label, passed in checks.items():
        print(f"  {'ok  ' if passed else 'FAIL'}  {label}")
    if all(checks.values()):
        print("pty reframe cancel acceptance passed")
        return 0
    print("pty reframe cancel acceptance failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    sys.exit(main())
