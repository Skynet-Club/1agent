# MCP OAuth Claims Policy — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `mcp_auth::authorize` and `1agent mcp-token-check`.

## Commands

```bash
cargo test mcp_auth --lib
cargo test --test mcp_token_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 4 tests
test mcp_auth::tests::audience_expiry_and_skew_are_enforced ... ok
test mcp_auth::tests::malformed_claims_are_rejected ... ok
test mcp_auth::tests::ttl_and_scope_are_enforced ... ok
test mcp_auth::tests::valid_claims_are_authorized ... ok

test result: ok. 4 passed; 0 failed

running 3 tests (tests/mcp_token_cli.rs)
test mcp_token_check_accepts_valid_claims ... ok
test mcp_token_check_rejects_audience_scope_ttl_and_expiry ... ok
test mcp_token_check_rejects_bad_claims_json ... ok

test result: ok. 3 passed; 0 failed
```

## What this proves

- Valid claims are authorized with the expected subject and audience.
- A token for another audience is rejected.
- Expiry and not-before honor the configured clock skew.
- A TTL above the policy maximum and a missing scope are rejected.
- Empty subjects, inverted lifetimes and an empty required scope are rejected.
- The CLI reads claims from a file, accepts valid claims, and prints
  `authorized: false` without the subject on rejection; malformed JSON fails.

## What this does not prove

- That the token signature was verified; the caller must do that first.
- OAuth issuer discovery, token exchange, refresh or revocation.
- MCP HTTP/SSE transport, SSE event handling or server authentication.
- That a trusted key/issuer configured by the caller is correct.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/mcp_auth.rs`
- `src/main.rs` (`mcp-token-check`)
- `tests/mcp_token_cli.rs`
- `docs/adr/0066-mcp-oauth-claims-policy.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
