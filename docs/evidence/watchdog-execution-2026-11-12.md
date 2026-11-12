# Watchdog Pause/Resume Execution — Evidence (2026-11-12)

Status: **passed on this machine**
Scope: explicit, checked SIGSTOP/SIGCONT after a Pause/Repair decision.

## Commands

```bash
cargo test --offline watchdog --lib
cargo test --offline --test watchdog_cli
```

## What this proves

- A real `sleep` child is paused with SIGSTOP and resumed with SIGCONT; the
  test polls `/proc/<pid>/status` for the stopped state.
- The executor refuses pid 0, its own pid, a missing pid and a command
  mismatch.
- The CLI refuses execution on `ok`/`warn` decisions, requires all three
  escalation thresholds, and prints `executed pause pid=...`.
- In JSON mode the decision remains the only stdout document; the receipt goes
  to stderr.
- The intervention log behavior is unchanged.

## What this does not prove

- Automatic repair or restart; the watchdog never kills or restarts.
- That SIGCONT repairs a hung process; it only resumes a stopped one.
- That the cmdline check is race-free; read and signal are separate syscalls.
- Windows/macOS signaling; the command fails closed there.
- That the selected default `first-principles-agent` matches every deployment
  binary name; operators can override `--expected-command`.
