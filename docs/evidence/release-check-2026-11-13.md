# Local Release Check — Evidence (2026-11-13)

Status: **local software gates pass; release blockers remain external/evidence**

Commit: `509061c` (clean tree)
Command: `./scripts/release-check.sh`
Exit: 0
Raw log: `docs/evidence/release-check-2026-11-13.log`
Raw log sha256: `d5e8adbfbc51185522b87198e20b41ae390a71c92a80134e924a81300513c350`

## Results

- `cargo fmt --all --check`: pass.
- `cargo clippy --offline --all-targets -- -D warnings`: pass.
- Full test suite: 1261 passed / 0 failed / 13 ignored.
- HTTP surface e2e: 1 passed.
- Local CI: status 0, commit `509061c` (clean), binary sha256
  `10d6a84ef47abb1cf9b01f819d47eeb15fec9a646ea99c923fd24fd5413a4b49`.
- Claims: 224 (200 verified / 16 partial / 8 no evidence); every ✅ evidence
  executed. The live terminal claim is partial because the local session
  daemon is at its 128-session limit (`session_limit`), not a code failure.
- TUI interaction: greeting, `/help`, `/status`, `/mode`, Ctrl+T, slash drawer,
  Tab completion, destructive confirmation, panel focus, backspace/Ctrl+Q and
  400 mixed edit events passed.
- Metrics gate: **skipped** because no `--metrics-dir` was passed.

## What this proves

- The local software gates listed above pass on this commit.
- The watchdog containment slice did not regress the full suite, CI, or TUI
  interaction.

## What this does not prove

- M1/M2/M3 8h long-horizon results; the benchmark is still running.
- M7 broader TTFT; the scheduled sample waits for the 8h benchmark.
- M19–M23 human labels; the packet still needs a reviewer.
- External A-Gate/W-Gate/security audit, macOS P-Gate, or a release package.
- The metrics gate; it was skipped without a representative journal directory.
