# Audit v0.91.0

Date: 2026-11-02

## Release state

- version: `0.91.0`
- feature commit: `0b3a6081370ac85571224e9d9a99d2a0020c7f42`
- tag: `v0.91.0`
- frozen terminal: not modified

## Verification snapshot

| Check | Result |
|---|---|
| Formatting | passed |
| Clippy with `-D warnings` | passed |
| Unit tests | 129 passed |
| Release-profile tests | 129 unit + all E2E passed |
| Offline integration / CLI E2E | passed |
| Core contract evaluation | 6/6 passed |
| Core hard-gate failures | 0 |
| Topbar keeps the latest instruction | passed |
| Running/completed state transition | passed |
| Terminal control sanitization | passed |
| Manual PTY workflow | passed |
| Clean-source package gate | passed |
| Linux/Windows archive reproducibility | passed |
| Release manifest and provenance | passed |
| Historical manifests v0.23–v0.91 | all verified |
| Packaged Linux binary smoke | passed |
| Git object integrity | passed |
| Locked dependency metadata | passed |
| Rust API documentation build | passed |

The packaged binary reported `1agent 0.91.0`, completed the six-case core suite at 6/6, and ran a
scripted TUI task with the expected persistent topbar and journal output.

## Requirement alignment

- The interface reuses the existing provider-neutral runtime and router.
- The second topbar line preserves the latest non-command instruction through execution and
  terminal states.
- Status, cognitive mode, risk, provider identity and budget remain visible.
- `/help`, `/clear`, `/exit` and `/quit` are implemented.
- Cancellation is routed through the existing runtime cancellation token.
- User and model text are sanitized before terminal rendering.

## Remaining evidence gaps

- live DeepSeek, Kimi, Qwen and Ollama quality;
- real reviewer calibration and held-out transfer;
- Windows TUI execution on a real Windows host;
- macOS runtime;
- cross-host hermetic reconstruction;
- raw-mode editing, scrolling, mouse and token streaming;
- cross-process TUI session restoration;
- asymmetric report identity and public trust anchors.

## Decision

Release the V0 presentation adapter while keeping model-quality and rich-terminal claims bounded
by the evidence above. The next interface requirement is resumable session identity plus explicit
route, evidence, ledger and report views.
