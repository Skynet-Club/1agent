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
| Clean-source package gate | passed |

## Verified clean release

```text
source commit: 0b9a44b24ae277920a78bfd8d8813d093891ccfb
git dirty: false

Linux binary:
1e6d4e2777c6544356179c800434d3c3c3f2b88fc54ba7974379f34cfa14b71a

Windows binary:
e9260109fb32a303bd670407d2d1fcad4a90bd75de11ec1f684b508e11d89bc3

Linux tar.gz:
c1b3fe03f2c5a60b96a369a1b08af1656ca35b900f5027ba086b4d78038ff6bf

Windows zip:
2940a6eea50e62d7b6cb9813267493f2060e5dfbc74e8b39fb5802836a044311

Release manifest:
6e958bfd2a412d7e379534ed6be1fab5fd7bdc4041098f1d97201543a2bad1a5
```

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
