# Handoff: v0.91.0

Date: 2026-11-02

## Current release

- version: `0.91.0`
- feature commit: `0b3a6081370ac85571224e9d9a99d2a0020c7f42`
- tag: `v0.91.0`
- repository status at handoff: clean

## Completed in this work cycle

- added `1agent tui` as a presentation adapter over the existing runtime;
- added a persistent topbar with status, mode, risk, provider and budget;
- preserved the latest non-command instruction through running and terminal states;
- added `/help`, `/clear`, `/exit` and `/quit`;
- routed Ctrl+C through runtime cancellation while preserving the journal;
- sanitized terminal-bound user and model text;
- added view-model tests and an executable CLI TUI E2E test;
- documented the interaction contract and presentation architecture.

## Verification evidence

- unit tests: 129 passed;
- release-profile tests: 129 unit tests and all E2E passed;
- offline integration and CLI E2E: passed;
- core contract evaluation: 6/6, zero hard-gate failures;
- Clippy with `-D warnings`: passed;
- clean-source package gate: passed;
- Linux and Windows archive reproducibility: passed;
- historical manifests v0.23–v0.91: verified;
- packaged Linux TUI smoke: passed;
- Rust API documentation build: passed.

Release hashes are recorded in [`status-v0.91.md`](status-v0.91.md).

## Run

```bash
cd /home/skynet/1-agent
cargo run --offline -- tui
```

With a provider configuration:

```bash
cargo run --offline -- tui --config configs/example.json
```

## Not verified

- live DeepSeek, Kimi, Qwen or Ollama quality, latency and cost;
- reviewed held-out model quality and calibration;
- Windows TUI execution on a real Windows host;
- macOS runtime;
- cross-host hermetic reconstruction;
- rich terminal modes beyond line-oriented ANSI rendering;
- cross-process TUI session restoration.

## Next action

Add resumable TUI session identity and explicit route, evidence, ledger and report views. Preserve
the current runtime contracts and require target-host evidence before claiming production model
quality or full rich-terminal portability.
