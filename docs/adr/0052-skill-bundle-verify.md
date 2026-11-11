# ADR 0052: Skill Bundle Verify Command

- Status: accepted
- Date: 2026-11-11

## Context

`skill export/import` verifies a bundle's SHA-256, schema, signature and
manifest hashes as part of import. A user receiving a bundle had no way to
check it before touching the skill registry.

## Decision

1. Extract `skill_bundle::verify_bundle(bytes, expected_sha256,
   trusted_public_key)` from the import path.
2. `verify_bundle` checks:
   - bundle size limit;
   - expected SHA-256;
   - JSON/schema version and non-empty skills;
   - Ed25519 signature when a trusted public key is supplied;
   - unsigned bundle + supplied key is an error;
   - duplicate name/version pairs and manifest body hashes;
   - the registry scan for every manifest (via an in-memory probe store).
3. `import_with_public_key` calls `verify_bundle` and then registers; it no
   longer duplicates the preflight.
4. Add `1agent skill verify --bundle PATH --sha256 HEX [--public-key PATH]
   [--json]`. It does not import or register anything.
5. A signed bundle requires `--public-key`; an unsigned bundle with
   `--public-key` is rejected rather than silently treated as trusted.

## Consequences

- A user can reject a tampered or unsafe bundle before import.
- CLI and import share one verification path, so they cannot drift.
- The command proves bundle integrity, not that the skill is safe to run;
  permissions, sandboxing and canary remain separate.

## Evidence

```bash
cargo test skill_bundle --lib
cargo test --test skill_verify_cli
```

The tests cover unsigned and signed bundles, a tampered bundle, and the
existing import tests continue to exercise the shared verifier.
