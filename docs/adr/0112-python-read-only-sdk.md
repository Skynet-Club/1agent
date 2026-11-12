# ADR 0112: Python Read-Only SDK

- Status: accepted
- Date: 2026-11-12
- Related: SA100-69, HTTP surface

## Context

The local HTTP surface had no client SDK. Callers had to hand-build request
JSON, protocol ids and bearer headers, and every caller could forget the
response-size or error-handling rules.

## Decision

1. Add `sdk/python/oneagent_client.py`, a standard-library client for
   `/v0/health`, `/v0/agent-card`, `/v0/trust-center` and `POST /v0/surface`.
2. The token comes from the `token` argument or `ONEAGENT_API_TOKEN`; the
   client never logs it and never prints request headers.
3. Responses are capped at 4 MiB. HTTP, transport and JSON failures raise
   `ClientError`; there is no silent retry. `surface` sends the `v0` protocol
   fields and an optional idempotency key.
4. Tests use a local `http.server` fake and cover read routes, surface
   protocol/echo, bearer and env-token behavior, HTTP/JSON errors, the size
   cap and URL validation.

## Consequences

- Python callers can use the surface without reimplementing protocol ids,
  auth headers or error handling.
- This is a read/surface client, not a full SDK: there is no task runner,
  streaming, retry policy, async client or typed model.
- The file is named `oneagent_client.py` because Python module names cannot
  start with a digit; the product remains 1Agent.
- The Python tests are run manually and are not part of `scripts/verify.sh`.
