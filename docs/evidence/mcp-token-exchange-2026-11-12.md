# MCP OAuth Token Exchange — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `mcp_exchange::exchange`, `1agent mcp-token-exchange`.

## Commands

```bash
cargo test mcp_exchange --lib
cargo test --test mcp_token_exchange_cli
cargo test tools::mcp --lib
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 2 tests
test mcp_exchange::tests::environment_names_and_values_fail_closed ... ok
test mcp_exchange::tests::invalid_configs_are_rejected_before_the_network ... ok

running 5 tests
test mcp_token_exchange_refuses_to_overwrite_the_output_file ... ok
test mcp_token_exchange_writes_a_private_token_file ... ok
test mcp_token_exchange_fails_closed_on_invalid_inputs ... ok
test mcp_token_exchange_does_not_follow_redirects ... ok
test mcp_token_exchange_discovers_the_token_endpoint ... ok
```

## What this proves

- The form contains the RFC 8693 grant type, subject token, audience and
  scope; the subject token is read only from the named environment variable.
- A loopback token endpoint returning a Bearer token with `expires_in: 300`
  produces a new output file with mode 0600 containing the token.
- Stdout contains the token type, TTL and SHA-256 but not the token.
- Missing environment variables, insecure remote HTTP, wrong token type,
  excessive TTL and partial client credentials fail closed.
- An existing output file is not overwritten.
- Invalid audience/scope/TTL/client-pair and insecure endpoints are rejected
  before any secret is read or request is sent.
- An MCP HTTP config with `token_exchange` exchanges once at spawn; the fake
  MCP server receives `Authorization: Bearer exchanged-token`.
- A 302 token endpoint is an error and the Location target receives no second
  connection.
- `--issuer` tries `oauth-authorization-server` (404), falls back to
  `openid-configuration`, discovers the loopback token endpoint and completes
  the exchange; a remote plain-HTTP issuer is rejected.
- A rotating endpoint returns `stale-token`, the MCP server rejects it with
  401, the client re-exchanges for `refreshed-token`, retries the request and
  completes initialization and discovery.
- With `expires_in: 31`, the token is used normally at spawn; after 1.5 s
  (inside the 30-second refresh window), the next tool call re-exchanges and
  sends `refreshed-token` without waiting for a 401.

## What this does not prove

- Interoperability with a real OAuth/IdP issuer or token endpoint.
- A retry budget, concurrent-refresh deduplication or issuer discovery; the
  proactive check is a fixed 30-second window.
- That the IdP enforces audience restriction; this validates the response
  shape and TTL, not the remote policy.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/mcp_exchange.rs`
- `src/main.rs` (`mcp-token-exchange`)
- `tests/mcp_token_exchange_cli.rs`
- `docs/adr/0083-mcp-token-exchange.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
