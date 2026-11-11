# ADR 0078: Agent Card Discovery Route

- Status: accepted
- Date: 2026-11-12

## Context

The Agent Card is served at `/v0/agent-card`, which is a local API path. A2A
peers look for a well-known location. Adding the standard path is a small step
toward interoperability without pretending to implement a registry.

## Decision

1. Serve the same Agent Card JSON at `GET
   /.well-known/agent-card.json` and `GET /v0/agent-card`.
2. Both routes are public read-only and do not require a bearer token.
3. The payload is the local card. If the card is unsigned, the well-known
   route is still unsigned; signing is a separate CLI step.
4. This is discovery only. Registry lookup, signed registry entries, key
   distribution and A2A task exchange are not implemented.

## Consequences

- A peer that knows the host can fetch the card without a local API path.
- The route exposes the same capability and limit claims as the local card;
  it does not make them true.
- There is no content negotiation, ETag or caching policy beyond the existing
  HTTP response.
- Hosting/TLS is the operator's responsibility.

## Evidence

```bash
cargo test http_surface --lib
```

The test fetches both routes without a bearer token and checks the product name
and non-empty capabilities.
