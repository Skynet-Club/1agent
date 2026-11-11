# Agent Card Discovery Route — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `GET /.well-known/agent-card.json` and `/v0/agent-card`.

## Commands

```bash
cargo test http_surface --lib
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running N tests (http_surface module)
test http_surface::tests::handler_serves_agent_card_without_bearer ... ok
... existing tests ...

test result: ok. N passed; 0 failed
```

## What this proves

- Both paths return status 200 without a bearer token.
- Both return the same `1Agent` card with non-empty capabilities.

## What this does not prove

- A2A registry discovery, signed registry entries or key distribution.
- That a remote peer can reach the host or that TLS is configured.
- That capability claims are operationally true.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/http_surface.rs`
- `docs/adr/0078-agent-card-discovery-route.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
