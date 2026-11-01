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
| Clippy `-D warnings` | passed |
| Clean-source package gate | passed |

## Known limitations

- The 8 MiB ceiling is a code constant, not yet a per-provider configuration.
- Successful JSON is fully buffered up to the ceiling before decoding.
- Loopback HTTP tests remain ignored in the managed sandbox; the live chunked transport path
  is implemented but has not been exercised in this environment.
- HTTP-error bodies are still represented by a short text excerpt, which may contain remote
  content.

## Verified clean release

```text
source commit: 00e479851b97a07446eb1fea7281fa9d3137c72c
git dirty: false

Linux binary:
f1d2ba278ce8084fe629c1c0c9c33bfcc5bd17c71d67505fd3bade1239c0f055

Windows binary:
b964373d6dfc8a4f3bbb53b147e335557a2361cbc2d13118840871349d4eb3cc

Linux tar.gz:
dca044271f49108507539b91c90a0156aaa6625fe961447185c79b46ecfbf65f

Windows zip:
e704e90674ee12c894e576864012989cddf70b901a9b89b8877f2b215ad293bb

Release manifest:
3cbb69f78076712fb3bb753519bbf1a9d8b90878ef8922d793e88399d03c7be8
```

## Next requirement

Sanitize remote provider error bodies:

1. stop echoing arbitrary non-2xx body excerpts;
2. expose only stable HTTP status and normalized error code;
3. optionally extract a bounded provider error identifier without free-form content;
4. ensure credentials and hidden control characters cannot enter error strings;
5. test authentication, rate-limit, server and HTML error responses.
