# ADR 0083: MCP OAuth Token Exchange

- Status: accepted
- Date: 2026-11-12

## Context

MCP HTTP servers can require an audience-restricted OAuth token rather than a
long-lived bearer secret. The client already validates claims and reads a
bearer token from an environment variable or a private file, but it had no
primitive to obtain a short-lived token.

## Decision

1. Add `mcp_exchange::exchange(TokenExchangeConfig)` implementing an RFC
   8693-style form POST:
   - `grant_type=urn:ietf:params:oauth:grant-type:token-exchange`;
   - `subject_token` and `subject_token_type=...:access_token`;
   - `audience` and `scope`;
   - optional `client_id`/`client_secret`.
2. Secrets are read only from named environment variables:
   `subject_token_env`, and optionally `client_id_env` +
   `client_secret_env` (both or neither). Errors name the variable, never its
   value.
3. The token endpoint must be HTTPS or loopback HTTP, with no embedded
   credentials. Redirects are not followed, the timeout is 5 seconds and the
   response body is capped at 1 MiB.
4. The response must contain a non-empty `access_token`, `token_type` equal to
   `Bearer` (case-insensitive) and `expires_in` in `1..=max_ttl_seconds`
   (default 3600, maximum 86400). A non-2xx status fails.
5. Add `1agent mcp-token-exchange --endpoint URL --audience A --scope S
   --subject-token-env VAR [--client-id-env VAR --client-secret-env VAR]
   [--max-ttl-seconds N] --out PATH [--json]`. The access token is written to
   a new 0600 file (no overwrite); stdout reports type, TTL and a SHA-256 of
   the token, never the token itself.
6. The resulting file can be used by `McpServerConfig.bearer_token_file`.
   Automatic refresh, issuer discovery and in-process exchange during MCP
   spawn remain future work.

## Consequences

- An operator can exchange a subject token and rotate the MCP token file
  without changing the daemon environment.
- A leaked stdout/log does not contain the token, only its hash.
- The exchange is manual: a token may expire while a long-running process is
  using it, and the caller must re-run the command. The MCP client does not
  retry or refresh.
- The client trusts the token endpoint's TLS and JSON; it validates TTL and
  bearer type, not the remote identity provider's signature.

## Evidence

```bash
cargo test mcp_exchange --lib
cargo test --test mcp_token_exchange_cli
cargo test tools::mcp --lib
```

The tests cover form fields, a 0600 output file, token non-disclosure, missing
environment variables, insecure endpoints, wrong token type, excessive TTL,
partial client credentials and refusal to overwrite an existing file.
