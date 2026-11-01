# Status v0.57.0

Date: 2026-11-02
Status: remote provider errors sanitized

## Changes since v0.56.0

- Non-2xx provider bodies are no longer read or excerpted.
- HTTP errors expose only the normalized status line and stable error code.
- Error mapping is now:

| HTTP status | Provider code | Retryable |
|---|---|---|
| 401, 403 | `authentication` | no |
| 408, 504 | `timeout` | yes |
| 429 | `rate_limited` | yes |
| 500-599 | `provider_unavailable` | yes |
| other non-2xx | `invalid_response` | no |

This prevents remote HTML, secret fragments, prompt-injection content and hidden controls from
flowing into provider errors or journals.

## Verification

| Check | Result |
|---|---|
| Unit tests | 99 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| Authentication classification | passed |
| Rate-limit classification | passed |
| Timeout classification | passed |
| Server-error classification | passed |
| Bad-request classification | passed |
| Error text excludes body markers | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- Provider-specific machine error identifiers are intentionally not extracted in this release.
- Transport errors may still include endpoint details supplied by the HTTP client.
- Retry remains provider-level; no global retry budget exists yet.
- Remote live behavior cannot be exercised through loopback in the managed sandbox.

## Next requirement

Sanitize transport errors and make retry semantics explicit:

1. avoid serializing full URLs or query strings in network errors;
2. preserve stable timeout/connect classifications;
3. verify authentication failures do not trigger fallback;
4. verify retryable failures do trigger fallback;
5. add error-disclosure tests for endpoints containing secret-looking query parameters.
