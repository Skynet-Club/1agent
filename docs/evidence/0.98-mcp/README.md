# 0.98 MCP Evidence

Date: 2026-11-15  
Gate: G3 (client conformance, synthetic + local real servers)

## Reproduce

```bash
cargo test --offline --lib tools::mcp
cargo test --offline --lib mcp_oauth
cargo test --offline --test mcp_real_servers -- --nocapture
```

Raw output: `tests.txt`.

## Result

- legacy `initialize` and modern 2026-07-28 `server/discover` dual era;
- modern `_meta`/`Mcp-Method`/`Mcp-Name`, no session header; `x-mcp-header`
  arguments mirrored as `Mcp-Param-*` (raw or Base64 sentinel);
- canonical tool-definition SHA-256 pinning fails closed on change;
- JSON Schema subset argument validation;
- HTTP public-IP resolution + loopback-only HTTP;
- OAuth 2.1 authorization-code + PKCE S256, RFC 8707 `resource`, RFC 9207
  `iss`, CIMD client metadata URL, 0600 token file;
- Tasks extension opt-in: bounded `tasks/get`, `input_required` fails closed,
  `tasks/update` is not guessed;
- MRTR `input_required` fails closed; URL elicitation exposes origin only and
  `requestState` is never echoed;
- real servers: Codex conformance server in modern 2026-07-28 and legacy
  2025-11-25 stdio modes, skynet-prompt stdio, and the conformance HTTP server
  checking `Mcp-Param-*` header/body consistency.

## Known gaps

- no remote OAuth server / authorization-server evidence;
- no live CIMD fetch test;
- in-task input (`tasks/update`) is not implemented;
- the real servers are local checkouts, not a third-party hosted service.
