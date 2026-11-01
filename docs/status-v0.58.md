# Status v0.58.0

Date: 2026-11-02
Status: transport errors sanitized and fallback semantics verified

## Changes since v0.57.0

- HTTP transport failures no longer embed `reqwest::Error` display text.
- Endpoint, query strings and credentials embedded in URLs cannot flow through this error path.
- Transport errors expose only:
  - `timeout` or `network`;
  - a fixed `provider transport failed` message;
  - explicit retryability.
- Timeout and connect failures are retryable; other transport failures are not.
- Runtime authentication failures are verified as non-retryable across providers.
- Runtime provider-unavailable failures remain verified as fallback-eligible.

## Verification

| Check | Result |
|---|---|
| Unit tests | 101 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| Transport message excludes URL/query | passed |
| Timeout classification | passed |
| Network classification | passed |
| Authentication no-fallback | passed |
| Retryable fallback | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- A custom in-process provider can still construct arbitrary error text.
- Journal records selected routes but provider failures are not yet persisted as events.
- No global retry budget or backoff policy exists.
- Live remote HTTP behavior remains unverified in the managed sandbox.

## Next requirement

Journal every provider failure before fallback or termination:

1. add a structured provider-failure event;
2. store provider/model, stable code, retryability and bounded message;
3. preserve attempt ordering in replay;
4. audit missing or contradictory failure outcomes;
5. update retryable and non-retryable runtime tests.
