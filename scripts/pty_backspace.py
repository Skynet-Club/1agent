#!/usr/bin/env python3
"""Regression check: Backspace must never terminate the TUI.

It also emulates the failure mode where a terminal maps Backspace to Ctrl+Q:
several Ctrl+Q presses only arm the explicit quit confirmation; Enter confirms,
Esc cancels.
"""
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

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BINARY = os.path.join(ROOT, "target/release/first-principles-agent")


def drain(fd, seconds):
    end = time.time() + seconds
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], 0.05)
        if not ready:
            continue
        try:
            data = os.read(fd, 65536)
        except OSError:
            return
        if not data:
            return


def exited(pid):
    waiter, status = os.waitpid(pid, os.WNOHANG)
    if waiter == 0:
        return None
    return os.waitstatus_to_exitcode(status)


def main():
    journal = tempfile.mkdtemp(prefix="1agent-backspace-")
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(
            BINARY,
            [BINARY, "tui", "--provider", "mock", "--journal-dir", journal],
        )
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))
    try:
        drain(fd, 1.8)
        os.write(fd, b"abc")
        drain(fd, 0.2)
        for _ in range(12):
            os.write(fd, b"\x7f")
            drain(fd, 0.05)
        drain(fd, 0.4)
        code = exited(pid)
        if code is not None:
            print(f"pty-backspace: FAIL (exited after Backspace: {code})", file=sys.stderr)
            return 1

        # Emulate a terminal that reports Backspace as Ctrl+Q. Repetition must
        # not bypass the explicit Enter confirmation.
        for _ in range(4):
            os.write(fd, b"\x11")
            drain(fd, 0.08)
        drain(fd, 0.3)
        code = exited(pid)
        if code is not None:
            print(f"pty-backspace: FAIL (Ctrl+Q exited without confirmation: {code})", file=sys.stderr)
            return 1
        os.write(fd, b"\x1b")
        drain(fd, 0.3)
        os.write(fd, b"\x11")
        drain(fd, 0.2)
        os.write(fd, b"\r")
        drain(fd, 1.0)
        code = exited(pid)
        if code != 0:
            print(f"pty-backspace: FAIL (explicit Enter did not exit: {code})", file=sys.stderr)
            return 1
        print("pty-backspace: passed")
        return 0
    finally:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except (ProcessLookupError, ChildProcessError):
            pass
        shutil.rmtree(journal, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
