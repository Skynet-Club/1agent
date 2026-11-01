# Status v0.27.0

Date: 2026-11-02
Status: provider credential and local-endpoint readiness validation implemented

## Changes since v0.26.0

- `api_key_env` is explicit:
  - when set, the named environment variable must exist before provider construction succeeds;
  - when omitted, no Authorization header is sent, supporting local Ollama or another
    unauthenticated endpoint.
- Empty credential variable names are rejected.
- Deterministic providers cannot declare credentials.
- Added `providers --check` to construct providers and validate endpoints without running a task.
- Readiness output contains manifests and status only; credential values are never serialized.
- Added child-process tests for missing and present credentials.

## Verification

| Check | Result |
|---|---|
| Missing declared credential | rejected before runtime |
| Present declared credential | ready |
| No-key local endpoint | allowed |
| Empty credential name | rejected |
| Secret value in stdout/stderr | absent |
| Unit tests | 69 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- Readiness does not contact remote endpoints; live authentication and model capability probes
  remain separate.
- Provider capability declarations remain operator supplied.
- Real DeepSeek, Kimi, Qwen and Ollama credentials are unavailable in this environment.
- Windows runtime execution remains unverified.

## Next requirement

Add an opt-in live provider conformance probe:

1. Send a bounded, non-sensitive structured-output request.
2. Verify authentication, timeout, response parsing and usage accounting.
3. Never persist credentials or full remote responses.
4. Make network execution explicitly opt-in.
5. Keep offline readiness separate from live conformance status.
