# ADR-0137: MCP dual-era transport, OAuth/CIMD and Tasks/MRTR gating

- Status: accepted for 0.98 (partial: no remote OAuth server evidence)
- Date: 2026-11-15
- Supersedes: none (extends ADR-0066 MCP OAuth claims, ADR-0070 Streamable HTTP)
- Related: `src/tools/mcp.rs`, `src/mcp_oauth.rs`,
  `docs/evidence/0.98-mcp/`, `tests/mcp_real_servers.rs`

## Context

0.97 spoke only legacy `initialize` + `tools/list`, trusted server tool
definitions after discovery, and had token-exchange OAuth only. The 2026-07-28
MCP revision makes the protocol stateless (`server/discover`, `_meta`,
per-request headers), adds MRTR `input_required`, and moves Tasks to the
`io.modelcontextprotocol/tasks` extension. Tool definitions are a supply-chain
input: a server can change a description or schema after the user approved it.

## Decisions

1. **Dual era.** Probe `server/discover` using the modern protocol version;
   on success cache modern 2026-07-28. Otherwise fall back through legacy
   `initialize` versions (2025-11-25, 2025-06-18, 2024-11-05).
2. **Modern request shape.** Every modern request carries
   `_meta.io.modelcontextprotocol/{protocolVersion,clientInfo,clientCapabilities}`
   plus `MCP-Protocol-Version`, `Mcp-Method` and, for tools, `Mcp-Name`.
   No `Mcp-Session-Id` is sent in modern mode.
3. **Argument header mirroring.** `x-mcp-header` schema annotations are sent
   as `Mcp-Param-<suffix>` headers. Values must be scalar; raw printable ASCII
   is sent directly, otherwise `=?base64?...?=` is used. Header names are
   validated and reserved/hop-by-hop names are rejected.
4. **Tool pinning.** The first connection writes a 0600 canonical SHA-256 map
   of full tool definitions (`pin_file`). Any add/remove/change fails closed
   until the operator reviews and deletes the pin file.
5. **Argument validation.** A conservative JSON Schema subset
   (type/required/additionalProperties/items/enum/numeric bounds) runs before
   the call; the remote schema is the authority after that.
6. **OAuth 2.1 authorization code + PKCE S256.** Loopback callback, RFC 8707
   `resource`, RFC 9207 `iss` validation, 0600 token file and refresh. CIMD is
   supported by setting `client_metadata_document` to an HTTPS metadata URL
   (path required, no fragment/userinfo); the operator hosts that document.
   DCR is not enabled.
7. **SSRF.** Non-loopback HTTP URLs are resolved and rejected unless every
   address is public; redirects are not followed.
8. **Tasks are opt-in and bounded.** `"tasks": true` declares the extension on
   modern requests. `resultType: "task"` polls `tasks/get` up to 120s/120
   polls with `pollIntervalMs` clamped; unknown statuses fail closed.
   `tasks/update` is not guessed.
9. **MRTR fails closed.** `resultType: "input_required"` returns
   `input_required` to the runtime; elicitation/sampling/roots are not
   auto-answered. URL elicitation reports only the origin. `requestState` is
   never parsed, logged or echoed.
10. **Annotations never lower local permission.** All MCP tools remain
    write-gated unless the local policy says otherwise; `destructiveHint`
    raises risk.

## Consequences

- A legacy-only server still works, at the cost of an extra failed probe.
- A CIMD deployment needs an operator-hosted metadata document; without it,
  the configured pre-registered `client_id` is used.
- Tasks can hide long-running work; they are off by default and bounded.
- Modern server evidence is local (Codex conformance server, skynet-prompt);
  a remote OAuth authorization server is still required before claiming full
  interoperability.

## Tests

- `src/tools/mcp.rs`: legacy stdio/HTTP, modern headers/no session,
  `Mcp-Param-*` mirroring, pinning, schema, SSRF config, Tasks disabled/poll/
  input-required, MRTR fail-closed.
- `src/mcp_oauth.rs`: PKCE RFC 7636 vector, CIMD validation, issuer mismatch,
  token expiry.
- `tests/mcp_real_servers.rs`: modern + legacy stdio, skynet-prompt stdio,
  modern HTTP `header_echo` header/body consistency.
