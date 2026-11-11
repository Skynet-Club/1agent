# ADR 0066: MCP OAuth Claims Policy

- Status: accepted
- Date: 2026-11-12

## Context

MCP servers may be reached with OAuth tokens. A token that is valid for one
server or one scope must not be accepted by another. SA98-55 calls for
audience-restricted, short-lived tokens.

## Decision

1. Add `McpToken { subject, audience, issued_at_ms, expires_at_ms, scopes }`
   and `McpTokenPolicy { expected_audience, max_ttl_ms, clock_skew_ms }`.
2. `authorize` fails closed when:
   - the subject is empty;
   - the audience is not exactly the expected audience;
   - `expires_at_ms <= issued_at_ms`;
   - the token is expired beyond the allowed clock skew;
   - the token is not valid yet beyond the allowed skew;
   - the lifetime exceeds `max_ttl_ms`;
   - the required scope is empty or absent from the token. Wildcards are not
     accepted.
3. `1agent mcp-token-check` reads claims from a JSON file (`--claims PATH`),
   never from argv, and takes `--audience`, `--scope`, `--max-ttl-ms`,
   `--skew-ms` and `--now-ms`. Rejection exits non-zero and does not print the
   subject.
4. The caller must verify the token signature with a trusted OAuth/JWT library
   before constructing the claims. This module is policy, not signature
   verification, token exchange or HTTP/SSE transport.

## Consequences

- A valid token for one MCP server cannot be replayed against another.
- Long-lived or overly broad tokens are rejected before a tool call.
- The claims file is sensitive operational input; it is read from disk and not
  echoed on rejection.
- A caller that skips signature verification can defeat the policy; this
  boundary is explicit.
- Key rotation, issuer discovery, OAuth token exchange and transport are
  separate slices.

## Evidence

```bash
cargo test mcp_auth --lib
cargo test --test mcp_token_cli
```

The tests cover valid claims, audience mismatch, expiry/not-before with skew,
TTL, missing scope, malformed claims, and a CLI rejection that does not echo
the subject.
