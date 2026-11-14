#!/usr/bin/env python3
"""PTY fuzz for composer editing: paste, spaces, backspace, arrows and mouse.

The real TUI is driven through a pseudo terminal with the deterministic
provider. The process must stay alive for the whole edit sequence; a panic or
silent exit fails the check.

    python3 scripts/pty_edit_stability.py
"""
import fcntl
import os
import pty
import random
import select
import struct
import sys
import tempfile
import termios
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BINARY = os.environ.get(
    "ONEAGENT_BIN", os.path.join(ROOT, "target/release/first-principles-agent")
)
PASTES = [
    "普通文本 with spaces",
    "多行\n粘贴\t内容 and 😀 emoji",
    "  leading and trailing  ",
    "é combining mark",
    "a" * 200,
    "长文本" * 12000,
    "\x1b[31mnot-an-escape",
]


def drain(fd, seconds):
    end = time.time() + seconds
    while time.time() < end:
        ready, _, _ = select.select([fd], [], [], 0.02)
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


def mouse(button, x, y, release=False):
    suffix = b"m" if release else b"M"
    return f"\x1b[<{button};{x};{y}".encode() + suffix


def main():
    if not os.access(BINARY, os.X_OK):
        print("build the release binary first", file=sys.stderr)
        return 1
    journal = tempfile.mkdtemp(prefix="1agent-edit-fuzz-")
    pid, fd = pty.fork()
    if pid == 0:
        os.environ["TERM"] = "xterm-256color"
        os.execv(
            BINARY,
            [BINARY, "tui", "--provider", "mock", "--journal-dir", journal],
        )
    fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 32, 100, 0, 0))
    rng = random.Random(20261110)
    try:
        drain(fd, 1.8)
        for index in range(400):
            choice = rng.randrange(10)
            if choice == 0:
                text = rng.choice(PASTES).encode()
                os.write(fd, b"\x1b[200~" + text + b"\x1b[201~")
            elif choice == 1:
                os.write(fd, b"\x7f" * rng.randrange(1, 5))
            elif choice == 2:
                os.write(fd, b" " * rng.randrange(1, 4))
            elif choice == 3:
                os.write(fd, rng.choice([b"a", "中".encode(), b"x y"]))
            elif choice == 4:
                os.write(fd, rng.choice([b"\x1b[D", b"\x1b[C", b"\x1b[H", b"\x1b[F"]))
            elif choice == 5:
                os.write(fd, b"\x1b[3~")
            elif choice == 6:
                # SGR mouse down / drag / up over the composer area.
                x = rng.randrange(2, 90)
                y = 29
                os.write(fd, mouse(0, x, y))
                os.write(fd, mouse(32, min(95, x + rng.randrange(1, 15)), y))
                os.write(fd, mouse(0, min(95, x + 15), y, release=True))
            elif choice == 7:
                os.write(fd, b"\x15")  # Ctrl+U is not bound; must be harmless.
            elif choice == 8:
                os.write(fd, b"\x1b[<64;10;20M")  # scroll up
            else:
                os.write(fd, b"hi\r")  # submit a short deterministic turn
                drain(fd, 0.25)
            drain(fd, 0.01)
            code = exited(pid)
            if code is not None:
                print(
                    f"pty-edit-stability: FAIL (exited at step {index}: {code})",
                    file=sys.stderr,
                )
                return 1
        drain(fd, 0.5)
        code = exited(pid)
        if code is not None:
            print(f"pty-edit-stability: FAIL (exited after edits: {code})", file=sys.stderr)
            return 1
        print("pty-edit-stability: passed (400 mixed edit events)")
        return 0
    finally:
        try:
            os.kill(pid, 9)
            os.waitpid(pid, 0)
        except (ProcessLookupError, ChildProcessError):
            pass
        import shutil

        shutil.rmtree(journal, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
