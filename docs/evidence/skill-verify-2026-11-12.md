# Skill Bundle Verify — Evidence (2026-11-12)

Status: **passed on this machine**  
Scope: `skill_bundle::verify_bundle` and
`1agent skill verify --bundle PATH --sha256 HEX [--public-key PATH]`.
The command verifies integrity; it does not import or execute the skills.

## Commands

```bash
cargo test skill_bundle --lib
cargo test --test skill_verify_cli
cargo clippy --all-targets -- -D warnings
```

## Expected results

```text
running ... tests (skill_bundle)
...
test result: ok

running 2 tests (tests/skill_verify_cli.rs)
test skill_verify_accepts_unsigned_and_signed_bundles ... ok
test skill_verify_rejects_a_tampered_bundle ... ok

test result: ok. 2 passed; 0 failed
```

## What this proves

- An unsigned bundle verifies with `signed=false`.
- A signed bundle verifies only with the trusted public key and reports
  `signed=true`.
- A tampered bundle fails the SHA-256 check with `{ok:false,error}` and a
  non-zero exit.
- Import and `skill verify` share `verify_bundle`, so their preflight cannot
  drift.
- The verifier runs the registry scan on every manifest via an in-memory probe
  store.

## What this does not prove

- That a verified skill is safe to execute; permissions, sandboxing and canary
  are separate.
- That the trusted public key is authentic; key distribution is out of scope.
- That the bundle came from a specific publisher; there is no remote index.
- Cross-platform behavior; the tests ran on Linux only.

## Files

- `src/skill_bundle.rs` (`verify_bundle`, shared import path)
- `src/main.rs` (`skill verify`)
- `tests/skill_verify_cli.rs`
- `docs/adr/0052-skill-bundle-verify.md`
- `docs/architecture.md`, `docs/features.md`, `docs/features.en.md`,
  `docs/claims.md`, `CHANGELOG.md`
