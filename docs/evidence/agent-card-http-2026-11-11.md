# Agent Card over HTTP — Evidence (2026-11-11)

Status: **passed on this machine**  
Scope: `GET /v0/agent-card` on the public read-only HTTP Surface. The card is
still local and unsigned; this is not an A2A registry-signed card.

## Commands

```bash
cargo test http_surface --lib
cargo test agent_card --lib
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running ... tests (http_surface)
test http_surface::tests::handler_serves_agent_card_without_bearer ... ok
...
test result: ok
```

## What this proves

- `GET /v0/agent-card` returns 200 without a bearer token.
- The response is the same `AgentCard` payload as the CLI: name `1Agent` and a
  non-empty capability list.
- The route is read-only and accepts no body.
- The card still marks unsupported/untested capabilities honestly.

## What this does not prove

- That the route is served by a live `1agent serve` process in production; the
  test calls the HTTP handler directly.
- That the card is signed or discoverable through A2A; it is local/unsigned.
- Cross-platform behavior; the test ran on Linux only.

## Files

- `src/http_surface.rs` (`/v0/agent-card`, test)
- `docs/adr/0051-serve-agent-card.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
