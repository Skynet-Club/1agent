import fcntl
import os
import pty
import re
import select
import struct
import subprocess
import sys
import termios
import time

BIN = sys.argv[1]
ROOT = sys.argv[2]
CWD = sys.argv[3]
RAW_OUT = sys.argv[4]
PLAIN_OUT = sys.argv[5]
PORT = sys.argv[6]
SCENARIO = sys.argv[7] if len(sys.argv) > 7 else "fresh"

ANSI = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07]*\x07")


def plain(text):
    text = ANSI.sub("", text)
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return "\n".join(line.rstrip() for line in text.splitlines())


def main():
    cols = int(os.environ.get("WIZ_COLS", "100"))
    rows = int(os.environ.get("WIZ_ROWS", "32"))
    master, slave = pty.openpty()
    fcntl.ioctl(
        slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0)
    )
    env = os.environ.copy()
    env["TERM"] = "xterm-256color"
    env["COLUMNS"] = str(cols)
    env["LINES"] = str(rows)
    if os.environ.get("WIZ_NO_COLOR"):
        env["NO_COLOR"] = "1"
    else:
        env.pop("NO_COLOR", None)
    if SCENARIO == "key":
        extra = ["--skip-probe", "--skip-smoke"]
    elif SCENARIO == "probe_fail":
        extra = ["--skip-smoke"]
    else:
        extra = []
    proc = subprocess.Popen(
        [BIN, "setup", "--root", ROOT, *extra],
        stdin=slave,
        stdout=slave,
        stderr=slave,
        cwd=CWD,
        env=env,
        close_fds=True,
    )
    os.close(slave)
    buffer = bytearray()
    cursor = 0

    if SCENARIO == "fresh":
        steps = [
            ("Press Enter to begin", b"\r", "welcome"),
            ("How should 1Agent connect?", b"2\r", "provider=openai-compatible"),
            ("Where is the OpenAI-compatible endpoint?", b"2\r", "endpoint=custom"),
            (
                "Endpoint URL",
                f"http://127.0.0.1:{PORT}/v1\r".encode(),
                f"endpoint=http://127.0.0.1:{PORT}/v1",
            ),
            ("Choose a model", b"\x1b[B\r", "model=fake-model-b"),
            ("How much may 1Agent do?", b"\r", "permissions=safe"),
            ("Where should 1Agent work?", b"\r", "workspace=current"),
            ("Ready to configure 1Agent?", b"\r", "run setup"),
        ]
    elif SCENARIO == "existing":
        steps = [
            ("Press Enter to begin", b"\r", "welcome"),
            ("How should 1Agent connect?", b"\r", "provider=openai-compatible"),
            ("Where is the OpenAI-compatible endpoint?", b"\r", "endpoint=keep"),
            ("Choose a model", b"\x1b[B\r", "model=fake-model-a"),
            ("How much may 1Agent do?", b"\r", "permissions=safe"),
            ("Where should 1Agent work?", b"\r", "workspace=current"),
            ("Ready to configure 1Agent?", b"\r", "run setup"),
        ]
    elif SCENARIO == "probe_fail":
        steps = [
            ("Press Enter to begin", b"\r", "welcome"),
            ("How should 1Agent connect?", b"2\r", "provider=openai-compatible"),
            ("Where is the OpenAI-compatible endpoint?", b"2\r", "endpoint=custom"),
            ("Endpoint URL", b"http://127.0.0.1:1/v1\r", "endpoint=unreachable"),
            ("Choose how to continue", b"\x1b[B\r", "probe failed -> manual"),
            ("Model id", b"manual-model\r", "model=manual-model"),
            ("How much may 1Agent do?", b"\r", "permissions=safe"),
            ("Where should 1Agent work?", b"\r", "workspace=current"),
            ("Ready to configure 1Agent?", b"\r", "run setup"),
        ]
    elif SCENARIO == "existing_same":
        steps = [
            ("Press Enter to begin", b"\r", "welcome"),
            ("How should 1Agent connect?", b"\r", "provider=keep"),
            ("Where is the OpenAI-compatible endpoint?", b"\r", "endpoint=keep"),
            ("Choose a model", b"\r", "model=keep"),
            ("How much may 1Agent do?", b"\r", "permissions=keep"),
            ("Where should 1Agent work?", b"\r", "workspace=keep"),
            ("Ready to configure 1Agent?", b"\r", "run setup"),
        ]
    elif SCENARIO == "cancel":
        steps = [
            ("Ctrl-C", b"\x03", "cancel at welcome"),
        ]
    elif SCENARIO == "key":
        steps = [
            ("Press Enter to begin", b"\r", "welcome"),
            ("How should 1Agent connect?", b"2\r", "provider=openai-compatible"),
            ("Where is the OpenAI-compatible endpoint?", b"2\r", "endpoint=custom"),
            ("Endpoint URL", b"https://example.com/v1\r", "endpoint=example"),
            ("OpenAI-compatible API key", b"fixture-key-123\r", "key=masked"),
            ("Choose a model", b"\r", "model=manual"),
            ("Model id", b"manual-model\r", "model=manual-model"),
            ("How much may 1Agent do?", b"\r", "permissions=safe"),
            ("Where should 1Agent work?", b"\r", "workspace=current"),
            ("Ready to configure 1Agent?", b"\r", "run setup"),
        ]
    else:
        raise SystemExit(f"unknown scenario {SCENARIO}")

    print(f"spawned pid={proc.pid}", flush=True)
    for marker, keys, label in steps:
        deadline = time.time() + 20
        found = False
        while time.time() < deadline:
            ready, _, _ = select.select([master], [], [], 0.2)
            if ready:
                try:
                    chunk = os.read(master, 65536)
                except OSError:
                    chunk = b""
                if not chunk:
                    break
                buffer.extend(chunk)
            text = buffer[cursor:].decode("utf-8", "replace")
            if marker in text:
                cursor += text.index(marker) + len(marker)
                found = True
                break
        if not found:
            print(f"TIMEOUT waiting for {marker!r} ({label})", flush=True)
            break
        print(f"-> {label}", flush=True)
        os.write(master, keys)
        time.sleep(0.15)

    # Drain until the child exits.
    deadline = time.time() + 60
    while proc.poll() is None and time.time() < deadline:
        ready, _, _ = select.select([master], [], [], 0.2)
        if ready:
            try:
                chunk = os.read(master, 65536)
            except OSError:
                chunk = b""
            if not chunk:
                break
            buffer.extend(chunk)
    try:
        while True:
            chunk = os.read(master, 65536)
            if not chunk:
                break
            buffer.extend(chunk)
    except OSError:
        pass
    os.close(master)
    code = proc.wait(timeout=10)
    raw = bytes(buffer).decode("utf-8", "replace")
    with open(RAW_OUT, "w", encoding="utf-8") as handle:
        handle.write(raw)
    with open(PLAIN_OUT, "w", encoding="utf-8") as handle:
        handle.write(plain(raw))
    expected = 130 if SCENARIO == "cancel" else 0
    print(f"exit={code} expected={expected}", flush=True)
    print(f"raw={RAW_OUT}", flush=True)
    print(f"plain={PLAIN_OUT}", flush=True)
    return 0 if code == expected else 1


if __name__ == "__main__":
    raise SystemExit(main())
