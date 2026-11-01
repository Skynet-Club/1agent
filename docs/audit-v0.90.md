# Audit v0.90.0

Date: 2026-11-02

## Release state

- version: `0.90.0`
- feature commit: `bf393a15c4c8bd4f234282bc21ab3d35e8d40b89`
- tag: `v0.90.0`
- frozen terminal: not modified

## Verification snapshot

| Check | Result |
|---|---|
| Formatting | passed |
| Clippy with `-D warnings` | passed |
| Unit tests | 127 passed |
| Offline integration / CLI E2E | passed |
| Core contract evaluation | 6/6 passed |
| Core hard-gate failures | 0 |
| Clean-source package gate | passed |
| Linux/Windows archive reproducibility | passed |
| Release manifest and Linux provenance | passed |
| Historical manifests v0.71–v0.90 | all verified |

## Security consistency

Provider construction and readiness preflight now apply the same rule: bearer credentials require
HTTPS for non-loopback endpoints. Remote plaintext HTTP with a declared key is rejected before a
run; unauthenticated loopback HTTP remains available for local hosted models.

## Remaining evidence gaps

- live DeepSeek, Kimi, Qwen and Ollama quality;
- real reviewer calibration and held-out transfer;
- Windows real-host execution;
- cross-host hermetic reconstruction;
- macOS runtime;
- asymmetric report identity and public trust anchors.

## Decision

Keep the V0 architecture and continue with
[`next-phase-evidence.md`](next-phase-evidence.md). Production model-quality claims remain
unsupported by current evidence.
