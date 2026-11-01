# Status v0.56.0

Date: 2026-11-02
Status: bounded provider responses implemented

## Changes since v0.55.0

- OpenAI-compatible providers cap response bodies at 8 MiB.
- Declared `Content-Length` above the cap is rejected before body accumulation.
- Streamed responses are accumulated incrementally and rejected as soon as the cap is crossed.
- The same cap applies to success and HTTP-error responses.
- Oversized bodies produce `ProviderErrorCode::ResponseTooLarge`.
- Oversized errors do not include response content.
- Response-body reads remain cancellable.

## Verification

| Check | Result |
|---|---|
| Unit tests | 98 passed |
| Integration / E2E | passed offline |
| Core evaluation | 6/6 passed |
| Declared oversized length | rejected |
| Chunked accumulation over limit | rejected |
| Exact limit | accepted |
| `response_too_large` serialization | passed |
| Response content excluded from size error | passed |
| Clippy `-D warnings` | pending |
| Clean-source package gate | pending |

## Known limitations

- The 8 MiB ceiling is a code constant, not yet a per-provider configuration.
- Successful JSON is fully buffered up to the ceiling before decoding.
- Loopback HTTP tests remain ignored in the managed sandbox; the live chunked transport path
  is implemented but has not been exercised in this environment.
- HTTP-error bodies are still represented by a short text excerpt, which may contain remote
  content.

## Next requirement

Sanitize remote provider error bodies:

1. stop echoing arbitrary non-2xx body excerpts;
2. expose only stable HTTP status and normalized error code;
3. optionally extract a bounded provider error identifier without free-form content;
4. ensure credentials and hidden control characters cannot enter error strings;
5. test authentication, rate-limit, server and HTML error responses.
