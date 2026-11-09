# ADR 0027: External Alert Webhook

- Status: accepted for the operator-notification slice
- Date: 2026-11-08

## Context

Provider health alerts were only appended to a local JSONL file. A release
operator needs an external notification path without changing the runtime's
local audit behavior or leaking credentials into the journal.

## Decision

1. `--alert-webhook URL` configures a best-effort JSON POST of `AlertRecord`
   from the `run` and `serve` runtime paths.
2. HTTPS is required. Plain HTTP is accepted only for a loopback host
   (`localhost` or a loopback IP), so local integrations and tests do not need
   TLS while remote delivery cannot be downgraded silently.
3. The client has a 5-second timeout and the runtime also wraps delivery in a
   5-second timeout. Delivery failure never blocks the local alert log or the
   provider fallback; the error is not journaled and does not echo the endpoint.
4. The webhook receives the same validated alert fields as the local log:
   provider id, model, failure count, threshold, message and timestamps. No API
   keys, request headers or prompt content are included.
5. The endpoint URL is redacted from `Debug` output. Webhook errors strip the
   URL before formatting.
6. This is a notification channel, not a delivery guarantee. There is no retry
   queue, signing, deduplication across restarts, or receiver authentication
   beyond the URL itself.

## Consequences

- Operators can receive provider health alerts outside the host with one flag.
- A remote `http://` endpoint is rejected; users must expose HTTPS or use a
  loopback relay.
- A webhook outage leaves the local alert log intact but loses that external
  notification. A durable outbox/retry design remains future work.
