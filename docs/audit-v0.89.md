# Audit v0.89.0

Date: 2026-11-02

## Release state

- version: `0.89.0`
- feature commit: `379a8d6501011ba0a9b301973169fb4683da9105`
- tag: `v0.89.0`
- repository at audit start: clean
- frozen terminal: not modified

## Verification snapshot

| Check | Result |
|---|---|
| Formatting | passed |
| Clippy with `-D warnings` | passed |
| Unit tests | 126 passed |
| Release-profile tests | 126 unit + all E2E passed |
| Offline integration / CLI E2E | passed |
| Core contract evaluation | 6/6 passed |
| Core hard-gate failures | 0 |
| Clean-source package gate | passed |
| Linux/Windows archive reproducibility | passed |
| Release manifest and Linux provenance | passed |
| Historical manifests v0.71–v0.89 | all verified |

## Security change

The OpenAI-compatible provider now rejects:

- bearer credentials sent to non-loopback plaintext HTTP;
- empty API keys;
- user/password credentials embedded in endpoint URLs.

Loopback HTTP remains available for local providers such as Ollama. HTTPS remains the transport
for non-loopback credential-bearing endpoints.

## Requirement status

Implemented and verified offline:

- provider-neutral task/model/tool/answer contracts;
- result-first answer contract with evidence and uncertainty accounting;
- decision-completeness evaluation;
- capability routing and bounded fallback;
- durable journal, audit, checkpoint and resume;
- gated L2 experience evolution;
- report policy/integrity/digest/HMAC/trust inspection;
- safe provider credential transport.

Still not verified:

- live DeepSeek, Kimi, Qwen or Ollama quality;
- real held-out model quality, calibration and positive experience transfer;
- Windows execution on a real host;
- cross-machine hermetic reconstruction;
- macOS runtime;
- asymmetric report identity and public trust anchors.

## Decision

The V0 runtime is internally consistent and release-verifiable. The next required step is the
environment-evidence plan in [`next-phase-evidence.md`](next-phase-evidence.md), not additional
autonomy.
