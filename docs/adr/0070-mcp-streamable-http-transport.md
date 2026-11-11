# ADR 0070: MCP Streamable HTTP Transport

- Status: accepted
- Date: 2026-11-12

## Context

The MCP client only spawned stdio servers. SA98-46 calls for HTTP/SSE
transport, tool discovery and audited calls. Remote MCP servers also need a
credential path that does not put tokens in a config file or argv.

## Decision

1. `McpServerConfig` accepts exactly one of:
   - `command: [program, args...]` for stdio;
   - `url: https://...` for streamable HTTP.
2. HTTP policy:
   - HTTPS is required except for loopback (`localhost` or a loopback IP);
   - URLs with embedded credentials are rejected;
   - a bearer token is read from the environment variable named by
     `bearer_token_env`, from a 0600 regular non-symlink file named by
     `bearer_token_file`, or exchanged once at spawn via `token_exchange`;
     setting more than one source is an error;
   - the token is never stored in the config, logged or included in errors;
   - a missing, empty, loose-permission or symlinked token source fails
     closed.
3. Requests are JSON-RPC POSTs with `Accept: application/json,
   text/event-stream`. The client records `Mcp-Session-Id`, sends it on later
   requests and accepts either a JSON response or SSE `data:` events. Batched
   JSON arrays are searched for the matching request id.
4. Notifications are POSTed without an id and accept any 2xx status.
5. Non-2xx responses fail with a bounded body excerpt. The error never prints
   the bearer token.
6. Existing stdio configs keep working; `command` becomes optional only so
   that `url` can replace it.

## Consequences

- A remote MCP server can be used without a child process.
- Local MCP servers can use plain HTTP on loopback; remote plain HTTP is
  rejected.
- SSE support parses `data:` events and ignores other fields; reconnection,
  resumability and server-initiated streams are not implemented.
- OAuth discovery is not implemented; the caller supplies a token through the
  environment, a private file or a spawn-time exchange. A 401 triggers one
  re-exchange when `token_exchange` is configured; proactive expiry refresh is
  not implemented. Claims can be validated with `mcp_auth`.
- Transport tests use a local fake HTTP server, not a third-party MCP server.

## Evidence

```bash
cargo test tools::mcp --lib
```

The tests cover stdio compatibility, HTTP initialize/list/call, SSE parsing,
rejection of both/neither transports, remote plain HTTP and embedded
credentials, and env-vs-file bearer token handling including unsafe files.
