# MCP stdio evidence — 2026-11-09

Provider: `openai-compatible` / `deepseek-flash`  
Host: Linux x86_64  
Command shape: `1agent run --mcp-config <config.json> --task "<objective>" --json`

## Discovery and tool call

The config declared one stdio server (`python3 server.py`) exposing an `echo`
tool with `readOnlyHint: true`. 1Agent performed the initialize handshake,
discovered the tool as `mcp_demo_echo`, and called it.

Observed tool call:

```text
mcp_demo_echo
```

Result: `stop_reason=completed`; the tool returned `echo:hello-mcp`. The model
reported the actual tool output, not a fabricated answer.

## Artifact

| File | SHA-256 |
|---|---|
| `echo-run.jsonl` | `9a10a840a898c569fb929ac5c360d755c818b6bfb73b504edb0975742b9bcdcf` |

## Caveats

- The live server is a local fake used to verify the transport; no third-party
  MCP server was exercised.
- Tool schemas are passed through permissively; the MCP server remains the
  validator. Write tools default to `requires_write=true` unless the server
  declares `readOnlyHint: true`.
- stdio only; HTTP/SSE MCP transports are not implemented yet.
