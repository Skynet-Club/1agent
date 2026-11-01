# Status v0.28.0

Date: 2026-11-02
Status: opt-in live provider conformance probing implemented

## Changes since v0.27.0

- Added `providers --probe --allow-network`.
- Probes only OpenAI-compatible providers; deterministic offline providers are excluded.
- Each probe sends a fixed, non-sensitive request using the canonical agent answer contract.
- A probe validates:
  - provider identity;
  - HTTP authentication/network/timeout behavior;
  - final structured `AgentAnswer` parsing and contract validation;
  - input/output token usage.
- Reports contain provider/model, success, latency, token counts, answer status and normalized
  error code.
- Complete remote responses and credential values are never persisted or printed.
- Offline `--check` remains a separate readiness state.

## Verification

| Check | Result |
|---|---|
| Network opt-in required | implemented |
| Valid structured answer probe | passed with deterministic provider |
| No tool call when no tools offered | rejected |
| Sanitized result contract | implemented |
| Unreachable endpoint report | normalized `network` error, no raw response |
| Live loopback HTTP bind | blocked by managed sandbox |
| Unit tests | 70 passed |
| Core evaluation rubric | 4/4 passed, 0 hard-gate failures |

## Known limitations

- No real DeepSeek, Kimi, Qwen or Ollama credentials/services are available for live validation.
- The managed sandbox blocks loopback binding, so successful live HTTP conformance remains
  unverified here.
- A successful probe proves protocol conformance for one request, not reasoning quality.
- The probe does not yet exercise tool calling.
- Live results are not persisted as evaluation evidence.

## Next requirement

Extend conformance coverage to tool calling without weakening sandboxing:

1. Offer one inert echo-like tool in a separate opt-in probe mode.
2. Validate call ID, JSON arguments and tool-observation round trip.
3. Never grant filesystem or network authority to the probe.
4. Record only normalized capability results.
5. Keep argument validation and authorization outside the model.
