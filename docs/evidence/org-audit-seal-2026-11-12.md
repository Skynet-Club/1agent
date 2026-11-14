# Organization Audit Seals — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `org_audit::seal`, `org_audit::verify_seal` and
`1agent org-audit seal|verify`.

## Commands

```bash
cargo test org_audit --lib
cargo test --test org_audit_seal_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running 5 tests (org_audit module)
test org_audit::tests::seals_bind_the_entry_count_and_head ... ok
... existing chain tests ...

test result: ok. 5 passed; 0 failed

running 1 test (tests/org_audit_seal_cli.rs)
test seals_and_detects_appends_and_tampering ... ok

test result: ok. 1 passed; 0 failed
```

## What this proves

- An empty store can be sealed at genesis; an entry after sealing invalidates
  the seal.
- A fresh seal verifies with the matching public key.
- The wrong public key and a changed head hash are rejected.
- The CLI generates a private/public key pair, seals two entries, verifies both
  chain and seal, detects a third entry as stale, re-seals, and rejects a
  tampered seal.
- `--seal` without `--public-key` exits non-zero.

## What this does not prove

- That the verifier obtained the public key from a trustworthy channel.
- That log and seal cannot both be replaced by an attacker.
- Actor authentication or remote transparency.
- Periodic sealing or seal retention.
- Cross-platform behavior; tests ran on Linux only.

## Files

- `src/org_audit.rs`
- `src/main.rs` (`org-audit generate-key|seal|verify`)
- `tests/org_audit_seal_cli.rs`
- `docs/adr/0074-organization-audit-seals.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
