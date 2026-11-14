# ADR 0051: Serve the Agent Card over the HTTP Surface

- Status: accepted
- Date: 2026-11-11

## Context

ADR-0047 added a local `agent-card` command. A remote reader, dashboard or
future A2A peer should be able to fetch the same card without shell access.

The HTTP Surface already has public read-only routes (`/v0/health`,
`/v0/openapi`) and a bearer-protected `/v0/surface` route. The card contains
capability and limit statements, not user data.

## Decision

1. Add `GET /v0/agent-card` to `handle_http`.
2. The route is public and read-only, like `/v0/health` and `/v0/openapi`.
3. It returns the same `agent_card::build_card(env!("CARGO_PKG_VERSION"))`
   payload as the CLI.
4. No bearer is required and no request body is accepted.
5. The card is still local and unsigned. This is not an A2A agent card served
   with a registry signature, and it does not claim capabilities the platform
   registry marks unsupported/untested.

## Consequences

- A local dashboard or remote reader can compare builds over HTTP.
- The public route exposes capability/limit metadata only; no secrets, user
  data or task state.
- A later signing/A2A slice can change the content type or add a signature
  without changing the route.

## Evidence

```bash
cargo test http_surface --lib
```

The test fetches `/v0/agent-card` without a bearer, asserts 200 and checks the
card name and non-empty capabilities.
