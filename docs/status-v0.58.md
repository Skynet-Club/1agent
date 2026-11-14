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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- A custom in-process provider can still construct arbitrary error text.
- Journal records selected routes but provider failures are not yet persisted as events.
- No global retry budget or backoff policy exists.
- Live remote HTTP behavior remains unverified in the managed sandbox.

## Verified clean release

```text
source commit: b6e7cdfa99093ef62f827b85599b7b96a64f2747
git dirty: false

Linux binary:
4ce83962f914a4489db49e5fb8d7ab08456aa1d3d49021103ed2cb06b807deaa

Windows binary:
da1ea9805aede2299207d5da239b4d618e750b841717587e69478917264e5ec0

Linux tar.gz:
ba4abcd0a831e9600b5fa14b9e86528616c5f48f72e553562d3c301762971d26

Windows zip:
847eb663fea8cfbfa7866c506af34be64bfcba8dba2b08df3854d2b165716577

Release manifest:
11dbafa0556b659efa2fd75ed730f70ede0f62cc81e06c9ab9b94b76abbef151
```

## Next requirement

Journal every provider failure before fallback or termination:

1. add a structured provider-failure event;
2. store provider/model, stable code, retryability and bounded message;
3. preserve attempt ordering in replay;
4. audit missing or contradictory failure outcomes;
5. update retryable and non-retryable runtime tests.
